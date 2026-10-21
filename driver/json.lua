-- Minimal JSON encode/decode (no external deps), enough for the small request
-- and response bodies this driver handles. Not a full RFC parser; it covers
-- objects, arrays, strings, numbers, booleans and null.

local Json = {}

local function skip_ws(s, i)
  local _, j = s:find("^[ \t\r\n]*", i)
  return (j or i - 1) + 1
end

local decode_value

local function decode_string(s, i)
  -- assumes s:sub(i,i) == '"'
  local buf, j = {}, i + 1
  while j <= #s do
    local c = s:sub(j, j)
    if c == '"' then
      return table.concat(buf), j + 1
    elseif c == "\\" then
      local e = s:sub(j + 1, j + 1)
      local map = { ['"'] = '"', ["\\"] = "\\", ["/"] = "/", b = "\b", f = "\f", n = "\n", r = "\r", t = "\t" }
      if map[e] then
        buf[#buf + 1] = map[e]; j = j + 2
      elseif e == "u" then
        local hex = s:sub(j + 2, j + 5)
        local cp = tonumber(hex, 16) or 0
        if cp < 128 then buf[#buf + 1] = string.char(cp) else buf[#buf + 1] = "?" end
        j = j + 6
      else
        buf[#buf + 1] = e; j = j + 2
      end
    else
      buf[#buf + 1] = c; j = j + 1
    end
  end
  return table.concat(buf), j
end

decode_value = function(s, i)
  i = skip_ws(s, i)
  local c = s:sub(i, i)
  if c == '"' then
    return decode_string(s, i)
  elseif c == "{" then
    local obj = {}
    i = skip_ws(s, i + 1)
    if s:sub(i, i) == "}" then return obj, i + 1 end
    while true do
      local key; key, i = decode_string(s, skip_ws(s, i))
      i = skip_ws(s, i)
      if s:sub(i, i) == ":" then i = i + 1 end
      local val; val, i = decode_value(s, i)
      obj[key] = val
      i = skip_ws(s, i)
      local sep = s:sub(i, i)
      if sep == "," then i = skip_ws(s, i + 1)
      else return obj, i + 1 end
    end
  elseif c == "[" then
    local arr = {}
    i = skip_ws(s, i + 1)
    if s:sub(i, i) == "]" then return arr, i + 1 end
    while true do
      local val; val, i = decode_value(s, i)
      arr[#arr + 1] = val
      i = skip_ws(s, i)
      local sep = s:sub(i, i)
      if sep == "," then i = i + 1
      else return arr, i + 1 end
    end
  elseif c == "t" then return true, i + 4
  elseif c == "f" then return false, i + 5
  elseif c == "n" then return nil, i + 4
  else
    local num = s:match("^%-?%d+%.?%d*[eE]?[%+%-]?%d*", i)
    return tonumber(num), i + #(num or "")
  end
end

function Json.decode(s)
  if type(s) ~= "string" or s == "" then return nil end
  local ok, val = pcall(function() return (decode_value(s, 1)) end)
  if ok then return val end
  return nil
end

local function encode_value(v)
  local t = type(v)
  if t == "nil" then return "null"
  elseif t == "boolean" then return tostring(v)
  elseif t == "number" then return tostring(v)
  elseif t == "string" then
    return '"' .. v:gsub('[%z\1-\31\\"]', function(ch)
      local m = { ['"'] = '\\"', ["\\"] = "\\\\", ["\n"] = "\\n", ["\r"] = "\\r", ["\t"] = "\\t" }
      return m[ch] or string.format("\\u%04x", ch:byte())
    end) .. '"'
  elseif t == "table" then
    -- array if 1..n contiguous
    local n, isarr = 0, true
    for k in pairs(v) do n = n + 1; if type(k) ~= "number" then isarr = false end end
    if isarr and n == #v then
      local parts = {}
      for _, item in ipairs(v) do parts[#parts + 1] = encode_value(item) end
      return "[" .. table.concat(parts, ",") .. "]"
    end
    local parts = {}
    for k, val in pairs(v) do
      parts[#parts + 1] = encode_value(tostring(k)) .. ":" .. encode_value(val)
    end
    return "{" .. table.concat(parts, ",") .. "}"
  end
  return "null"
end

function Json.encode(v)
  local ok, s = pcall(encode_value, v)
  if ok then return s end
  return "null"
end

return Json
