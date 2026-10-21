-- Minimal mDNS: build a PTR query for a service type and parse the A/SRV/TXT
-- records from responses. Control4 gives only a raw UDP socket (no resolver),
-- so we hand-build/parse the wire format.
--
-- UNCERTAIN: the exact Control4 UDP-multicast socket API is not verified here
-- (see README). This module is transport-agnostic: Build() returns the query
-- bytes to send to 224.0.0.251:5353, and Parse() decodes a response buffer.

local MDns = {}

local function u16(n) return string.char(math.floor(n / 256) % 256, n % 256) end

-- Encode a dotted/labelled name "_appletv-bridge._tcp.local" to DNS labels.
local function encode_name(name)
  local out = {}
  for label in (name .. "."):gmatch("([^.]*)%.") do
    if #label > 0 then
      out[#out + 1] = string.char(#label) .. label
    end
  end
  out[#out + 1] = "\0"
  return table.concat(out)
end

-- A standard mDNS PTR query (QTYPE=12 PTR, QCLASS=1 IN) for the service type.
function MDns.Build(service_type)
  local header = u16(0) .. u16(0) .. u16(1) .. u16(0) .. u16(0) .. u16(0) -- id,flags,qd=1,an,ns,ar
  local q = encode_name(service_type) .. u16(12) .. u16(1)
  return header .. q
end

-- Read a (possibly compressed) name starting at offset i. Returns name, next.
local function read_name(buf, i)
  local labels, jumped, next_i = {}, false, nil
  while true do
    local len = buf:byte(i) or 0
    if len == 0 then i = i + 1; break end
    if len >= 192 then -- pointer (compression)
      local ptr = (len - 192) * 256 + (buf:byte(i + 1) or 0)
      if not jumped then next_i = i + 2 end
      jumped = true
      i = ptr + 1
    else
      labels[#labels + 1] = buf:sub(i + 1, i + len)
      i = i + len + 1
    end
  end
  return table.concat(labels, "."), (jumped and next_i) or i
end

-- Parse a response buffer; return a list of {name, ip, port, txt={k=v}}.
function MDns.Parse(buf)
  local results = {}
  if type(buf) ~= "string" or #buf < 12 then return results end
  local qd = (buf:byte(5) or 0) * 256 + (buf:byte(6) or 0)
  local an = (buf:byte(7) or 0) * 256 + (buf:byte(8) or 0)
  local ns = (buf:byte(9) or 0) * 256 + (buf:byte(10) or 0)
  local ar = (buf:byte(11) or 0) * 256 + (buf:byte(12) or 0)
  local total = an + ns + ar
  local i = 13
  -- skip questions
  for _ = 1, qd do
    local _, ni = read_name(buf, i); i = ni + 4
  end
  local byname = {}
  local function slot(n) byname[n] = byname[n] or { name = n, txt = {} }; return byname[n] end
  for _ = 1, total do
    local name, ni = read_name(buf, i)
    i = ni
    local rtype = (buf:byte(i) or 0) * 256 + (buf:byte(i + 1) or 0)
    local rdlen = (buf:byte(i + 8) or 0) * 256 + (buf:byte(i + 9) or 0)
    local rdata_i = i + 10
    local rdata = buf:sub(rdata_i, rdata_i + rdlen - 1)
    if rtype == 12 then -- PTR -> instance name
      local inst = read_name(buf, rdata_i)
      slot(inst)
    elseif rtype == 33 then -- SRV: prio(2) weight(2) port(2) target
      local s = slot(name)
      s.port = (rdata:byte(5) or 0) * 256 + (rdata:byte(6) or 0)
      s.target = read_name(buf, rdata_i + 6)
    elseif rtype == 16 then -- TXT: sequence of length-prefixed key=val
      local s, j = slot(name), 1
      while j <= #rdata do
        local l = rdata:byte(j) or 0
        local kv = rdata:sub(j + 1, j + l)
        local k, v = kv:match("^([^=]+)=(.*)$")
        if k then s.txt[k] = v end
        j = j + l + 1
      end
    elseif rtype == 1 then -- A record: IPv4
      local s = slot(name)
      if #rdata == 4 then
        s.ip = string.format("%d.%d.%d.%d", rdata:byte(1), rdata:byte(2), rdata:byte(3), rdata:byte(4))
      end
    end
    i = rdata_i + rdlen
  end
  for _, s in pairs(byname) do
    -- Prefer the A record resolved by target name if present.
    if not s.ip and s.target and byname[s.target] and byname[s.target].ip then
      s.ip = byname[s.target].ip
    end
    if s.ip or next(s.txt) then results[#results + 1] = s end
  end
  return results
end

return MDns
