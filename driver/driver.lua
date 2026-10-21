-- Apple TV Siri Remote Bridge — native Control4 driver.
--
-- This driver represents a Siri Remote bridge (a Pi or ESP32 running the
-- control4-appletv-remote firmware). The bridge POSTs resolved room commands
-- to this driver over HTTP (docs/FORWARDER.md); the driver forwards them to the
-- Control4 room(s) it is bound to via C4:SendToDevice. It also discovers
-- bridges on the LAN via mDNS so the installer need not type an IP.
--
-- It is NOT a cable/source proxy: a Siri Remote is an input that sends room
-- commands, so we bind to a Room and send commands, exactly as the Room Control
-- Webhook suite does (whose command set and SendToDevice approach we reuse).
-- Voice is a separate path (bridge -> appled), not handled here.

local Json = require("json")
local MDns = require("mdns")

local SERVICE_TYPE = "_appletv-bridge._tcp.local"
local MDNS_ADDR, MDNS_PORT = "224.0.0.251", 5353

local SERVER                 -- C4:CreateTCPServer handle
local CONTROL_ROOMS = {}     -- [bindingId] = roomDeviceId
local DISCOVERED = {}        -- [key] = {name, ip, port, room, model, id}
PersistData = PersistData or {}

-- ---------------------------------------------------------------- utilities
local function DebugLog(msg)
  if (Properties and Properties["Debug Mode"] == "On") then
    print("[appletv-remote] " .. tostring(msg))
  end
end

local function SetProp(name, value)
  if C4 and C4.UpdateProperty then C4:UpdateProperty(name, tostring(value)) end
end

-- Full Control4 room command allowlist + hold pairs (from the Room Control
-- Webhook command set). Human aliases map to the canonical command.
local HOLD_PAIRS = {
  START_VOL_UP = "STOP_VOL_UP", START_VOL_DOWN = "STOP_VOL_DOWN",
  START_CH_UP = "STOP_CH_UP", START_CH_DOWN = "STOP_CH_DOWN",
  START_SCAN_FWD = "STOP_SCAN_FWD", START_SCAN_REV = "STOP_SCAN_REV",
  START_PAGE_UP = "STOP_PAGE_UP", START_PAGE_DOWN = "STOP_PAGE_DOWN",
}
local ALIASES = {
  VOLUME_UP = "START_VOL_UP", VOLUME_DOWN = "START_VOL_DOWN",
  VOL_UP = "START_VOL_UP", VOL_DOWN = "START_VOL_DOWN",
  CHANNEL_UP = "START_CH_UP", CHANNEL_DOWN = "START_CH_DOWN",
  PLAY_PAUSE = "PLAYPAUSE", OK = "ENTER", SELECT = "ENTER",
  FAST_FORWARD = "START_SCAN_FWD", REWIND = "START_SCAN_REV",
  HOME = "ROOM_OFF",
}
local ALLOWED = {}
for _, c in ipairs({
  "PLAYPAUSE","PLAY","PAUSE","STOP","SKIP_FWD","SKIP_REV","RECORD","EJECT",
  "UP","DOWN","LEFT","RIGHT","ENTER","BACK","CANCEL","EXIT","MENU","GUIDE","INFO",
  "MUTE_ON","MUTE_OFF","MUTE_TOGGLE","ROOM_OFF","POWER","RECALL",
  "NUMBER_0","NUMBER_1","NUMBER_2","NUMBER_3","NUMBER_4","NUMBER_5","NUMBER_6","NUMBER_7","NUMBER_8","NUMBER_9",
  "CUSTOM_1","CUSTOM_2","CUSTOM_3",
}) do ALLOWED[c] = true end
for s, e in pairs(HOLD_PAIRS) do ALLOWED[s] = true; ALLOWED[e] = true end

local function CanonCommand(name)
  if not name then return nil end
  local up = tostring(name):upper()
  up = ALIASES[up] or up
  if ALLOWED[up] then return up end
  return nil
end

-- ------------------------------------------------------------- room routing
-- Resolve requested rooms: a room value may be a device id, a room name,
-- "all"/"*", or empty (= the single bound room). Returns a list of room ids.
local function BoundRoomIds()
  local ids = {}
  for _, id in pairs(CONTROL_ROOMS) do if id then ids[#ids + 1] = id end end
  return ids
end

local function FindRoomByName(name)
  -- C4:GetDeviceData / room lookup by name is version-specific; match against
  -- the rooms we are bound to using C4:GetDeviceDisplayName when available.
  if C4 and C4.GetDeviceDisplayName then
    for _, id in ipairs(BoundRoomIds()) do
      local ok, dn = pcall(function() return C4:GetDeviceDisplayName(id) end)
      if ok and dn and tostring(dn):lower() == tostring(name):lower() then return id end
    end
  end
  return nil
end

local function ResolveRooms(value)
  local bound = BoundRoomIds()
  if value == nil or value == "" then
    if #bound == 1 then return { bound[1] } end
    if #bound == 0 then return nil, "driver is not bound to a room" end
    return nil, "specify a room (bound to multiple)"
  end
  if value == "all" or value == "*" then return bound end
  local ids = {}
  for raw in tostring(value):gmatch("[^,]+") do
    local token = raw:gsub("^%s+", ""):gsub("%s+$", "")
    local id = tonumber(token) or FindRoomByName(token)
    if id then ids[#ids + 1] = id end
  end
  if #ids == 0 then return nil, "no matching room" end
  return ids
end

local function SendRoom(cmd, params, roomValue)
  local canon = CanonCommand(cmd)
  if not canon then return false, "unknown command: " .. tostring(cmd) end
  local rooms, err = ResolveRooms(roomValue)
  if not rooms then return false, err end
  for _, id in ipairs(rooms) do
    DebugLog("SendToDevice room=" .. id .. " cmd=" .. canon)
    pcall(function() C4:SendToDevice(id, canon, params or {}) end)
  end
  return true
end

-- --------------------------------------------------------------- HTTP layer
local function Unauthorized(headers, body)
  local pw = Properties and Properties["Password"] or ""
  if pw == "" then return false end
  local auth = headers["authorization"] or ""
  if auth == ("Bearer " .. pw) then return false end
  if (headers["x-api-key"] == pw) then return false end
  if body and body.password == pw then return false end
  return true
end

local function Route(method, path, headers, body)
  if path == "/health" then
    return 200, { ok = true, name = "Apple TV Siri Remote Bridge", rooms = #BoundRoomIds() }
  end
  if Unauthorized(headers, body) then
    return 401, { ok = false, error = "unauthorized" }
  end
  if path == "/commands" then
    local list = {}; for c in pairs(ALLOWED) do list[#list + 1] = c end
    return 200, { ok = true, commands = list }
  elseif path == "/rooms" then
    return 200, { ok = true, rooms = BoundRoomIds() }
  elseif path == "/command" then
    local ok, err = SendRoom(body.command, body.params, body.room)
    if ok then return 200, { ok = true } else return 400, { ok = false, error = err } end
  elseif path == "/preset" then
    -- Presets are a room source-selection concept; forward as-is for the room
    -- driver to interpret. UNCERTAIN: exact preset command name per install.
    pcall(function() for _, id in ipairs(select(1, ResolveRooms(body.room)) or {}) do
      C4:SendToDevice(id, "SELECT_PRESET", { PRESET = tostring(body.preset) })
    end end)
    return 200, { ok = true, note = "preset forwarded" }
  elseif path == "/button" then
    -- Custom room buttons: forward the button name; mapping is per-room config.
    return 200, { ok = true, note = "custom buttons are configured per room" }
  end
  return 404, { ok = false, error = "not found" }
end

-- Very small HTTP/1.1 request parser: request line + headers + optional body.
local function ParseRequest(raw)
  local head, body = raw:match("^(.-)\r\n\r\n(.*)$")
  if not head then head = raw; body = "" end
  local method, path = head:match("^(%u+)%s+(%S+)")
  local headers = {}
  for line in head:gmatch("[^\r\n]+") do
    local k, v = line:match("^([%w%-]+):%s*(.*)$")
    if k then headers[k:lower()] = v end
  end
  -- strip query string into a crude table merged with JSON body later
  local query = {}
  local p, qs = path and path:match("^([^?]*)%??(.*)$")
  if qs and qs ~= "" then
    for kv in qs:gmatch("[^&]+") do
      local k, v = kv:match("^([^=]+)=?(.*)$"); if k then query[k] = v end
    end
  end
  local obj = Json.decode(body) or {}
  for k, v in pairs(query) do if obj[k] == nil then obj[k] = v end end
  return method or "GET", p or "/", headers, obj
end

local function HandleClient(client)
  local buf = ""
  client:OnData(function(_, data)
    buf = buf .. tostring(data)
    if buf:find("\r\n\r\n", 1, true) then
      local method, path, headers, body = ParseRequest(buf)
      local status, payload = Route(method, path, headers, body)
      local json = Json.encode(payload)
      local resp = string.format(
        "HTTP/1.1 %d OK\r\nContent-Type: application/json\r\nContent-Length: %d\r\nConnection: close\r\n\r\n%s",
        status, #json, json)
      pcall(function() client:Write(resp) end)
      pcall(function() client:Close() end)
      buf = ""
    end
  end)
  client:OnClose(function() end)
  if client.Read then pcall(function() client:Read() end) end
end

local function StartServer()
  if not (C4 and C4.CreateTCPServer) then
    SetProp("Connection Status", "C4:CreateTCPServer unavailable"); return
  end
  local port = tonumber(Properties and Properties["Listen Port"]) or 5090
  SERVER = C4:CreateTCPServer()
  SERVER
    :OnListen(function() SetProp("Connection Status", "Listening on " .. port) end)
    :OnError(function(_, code, msg) SetProp("Connection Status", "Error " .. tostring(code) .. ": " .. tostring(msg)) end)
    :OnAccept(function(_, client) HandleClient(client) end)
  pcall(function() SERVER:Option("reuseaddr", true) end)
  local ok = pcall(function() return SERVER:Listen("0.0.0.0", port, 16) end)
  if not ok then SetProp("Connection Status", "Listen failed on " .. port); SERVER = nil end
end

local function StopServer()
  if SERVER and SERVER.Close then pcall(function() SERVER:Close() end) end
  SERVER = nil
end

-- ---------------------------------------------------------------- discovery
local DISCO_BINDING = 900
local function StartDiscovery()
  DISCOVERED = {}
  if not (C4 and C4.CreateNetworkConnection) then
    DebugLog("mDNS unavailable: no CreateNetworkConnection"); return
  end
  pcall(function()
    C4:CreateNetworkConnection(DISCO_BINDING, MDNS_ADDR, "udp")
    C4:NetConnect(DISCO_BINDING, MDNS_PORT)
    C4:SendToNetwork(DISCO_BINDING, MDNS_PORT, MDns.Build(SERVICE_TYPE))
  end)
  DebugLog("mDNS query sent for " .. SERVICE_TYPE)
end

function ReceivedFromNetwork(idBinding, _, strData)
  if idBinding ~= DISCO_BINDING then return end
  for _, b in ipairs(MDns.Parse(strData)) do
    local key = (b.txt and b.txt.id) or b.ip or b.name
    if key then
      DISCOVERED[key] = {
        name = (b.txt and b.txt.name) or b.name, ip = b.ip, port = b.port,
        room = b.txt and b.txt.room, model = b.txt and b.txt.model, id = b.txt and b.txt.id,
      }
      if C4 and C4.FireEvent then pcall(function() C4:FireEvent("Bridge Discovered") end) end
    end
  end
  -- Publish the list for the Composer dropdown.
  local items = {}
  for _, d in pairs(DISCOVERED) do
    items[#items + 1] = string.format("%s (%s)", d.name or "bridge", d.ip or "?")
  end
  if C4 and C4.UpdatePropertyList then pcall(function() C4:UpdatePropertyList("Discovered Bridges", items) end) end
end

local function UseSelectedBridge()
  local sel = Properties and Properties["Discovered Bridges"] or ""
  local ip = sel:match("%((%d+%.%d+%.%d+%.%d+)%)")
  if ip then
    SetProp("Bridge IP Address", ip)
    PersistData.bridge_ip = ip
    for _, d in pairs(DISCOVERED) do
      if d.ip == ip then SetProp("Bridge Name", d.name or ""); PersistData.bridge_name = d.name end
    end
    SetProp("Connection Status", "Bridge selected: " .. ip)
  end
end

-- ------------------------------------------------------------- C4 callbacks
function OnDriverLateInit()
  PersistData = PersistData or {}
  if PersistData.bridge_ip then SetProp("Bridge IP Address", PersistData.bridge_ip) end
  if PersistData.bridge_name then SetProp("Bridge Name", PersistData.bridge_name) end
  StartServer()
end

function OnDriverDestroyed()
  StopServer()
end

function OnBindingChanged(idBinding, _, bIsBound, otherDeviceID)
  local id = tonumber(otherDeviceID)
  if bIsBound and id then CONTROL_ROOMS[idBinding] = id
  else CONTROL_ROOMS[idBinding] = nil end
  DebugLog("binding " .. tostring(idBinding) .. " bound=" .. tostring(bIsBound) .. " room=" .. tostring(id))
end

function OnPropertyChanged(name)
  local v = Properties and Properties[name]
  if name == "Listen Port" then StopServer(); StartServer()
  elseif name == "Bridge IP Address" then PersistData.bridge_ip = v
  elseif name == "Discovered Bridges" then UseSelectedBridge()
  end
end

-- Composer actions and programming commands.
function ExecuteCommand(strCommand, tParams)
  if strCommand == "LUA_ACTION" and tParams and tParams.ACTION then strCommand = tParams.ACTION end
  if strCommand == "DiscoverBridges" then StartDiscovery()
  elseif strCommand == "UseSelectedBridge" then UseSelectedBridge()
  elseif CanonCommand(strCommand) then SendRoom(strCommand, tParams, tParams and tParams.room)
  end
end
