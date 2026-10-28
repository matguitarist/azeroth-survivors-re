-- Damage meter: damage done by you, your group and its pets (pets count for their owner), for the
-- current fight and overall. Read from the combat log's chat lines, matched against the client's own
-- GlobalStrings, so it follows the client's language.

local AP = AzerothPlus

local BARS, WIDTH, BAR_H = 8, 210, 16

-- Each combat-log string the meter reads, and what its captures are, in order.
local SELF = {
  { "SPELLLOGCRITSCHOOLSELFOTHER", "spell", "target", "amount", "school" },
  { "SPELLLOGSCHOOLSELFOTHER", "spell", "target", "amount", "school" },
  { "SPELLLOGCRITSELFOTHER", "spell", "target", "amount" },
  { "SPELLLOGSELFOTHER", "spell", "target", "amount" },
  { "COMBATHITCRITSCHOOLSELFOTHER", "target", "amount", "school" },
  { "COMBATHITSCHOOLSELFOTHER", "target", "amount", "school" },
  { "COMBATHITCRITSELFOTHER", "target", "amount" },
  { "COMBATHITSELFOTHER", "target", "amount" },
  { "PERIODICAURADAMAGESELFOTHER", "target", "amount", "school", "spell" },
}
local OTHER = {
  { "SPELLLOGCRITSCHOOLOTHEROTHER", "source", "spell", "target", "amount", "school" },
  { "SPELLLOGSCHOOLOTHEROTHER", "source", "spell", "target", "amount", "school" },
  { "SPELLLOGCRITOTHEROTHER", "source", "spell", "target", "amount" },
  { "SPELLLOGOTHEROTHER", "source", "spell", "target", "amount" },
  { "PERIODICAURADAMAGEOTHEROTHER", "target", "amount", "school", "source", "spell" },
  { "COMBATHITCRITSCHOOLOTHEROTHER", "source", "target", "amount", "school" },
  { "COMBATHITSCHOOLOTHEROTHER", "source", "target", "amount", "school" },
  { "COMBATHITCRITOTHEROTHER", "source", "target", "amount" },
  { "COMBATHITOTHEROTHER", "source", "target", "amount" },
}

-- A GlobalStrings format as a Lua pattern, and for each capture the argument it holds: `%s` and
-- `%d` count up, a positional `%2$s` names its own, since a translation may reorder them. Not
-- anchored at the end: the client appends resist, block and absorb notes.
local function pattern(format)
  local order, n = {}, 0
  local p = string.gsub(format, "%%(%d?)%$?([sd])", function(pos, kind)
    n = n + 1
    table.insert(order, tonumber(pos) or n)
    return "\001" .. kind
  end)
  p = string.gsub(p, "([%(%)%.%+%-%*%?%[%]%^%$%%])", "%%%1")
  p = string.gsub(p, "\001s", "(.-)")
  p = string.gsub(p, "\001d", "(%%d+)")
  return "^" .. p, order
end

local function compile(list)
  local out = {}
  for _, row in ipairs(list) do
    local format = getglobal(row[1])
    if format then
      local p, order = pattern(format)
      local roles = {}
      for i = 1, table.getn(order) do roles[i] = row[1 + order[i]] end
      table.insert(out, { pattern = p, roles = roles })
    end
  end
  return out
end

local selfPatterns, otherPatterns

-- The damage in `msg` as (source, amount), or nil; `source` nil means the player.
local function parse(msg, patterns)
  for _, p in ipairs(patterns) do
    local found = { string.find(msg, p.pattern) }
    if found[1] then
      local got = {}
      for i = 1, table.getn(p.roles) do got[p.roles[i]] = found[i + 2] end
      return got.source, tonumber(got.amount)
    end
  end
end

-- Who counts, by name: the player, the group, and every pet mapped to its owner.
local owners = {}
local function roster()
  owners = {}
  local function add(unit, pet)
    if UnitExists(unit) then
      local name = UnitName(unit)
      owners[name] = name
      if UnitExists(pet) then owners[UnitName(pet)] = name end
    end
  end
  add("player", "pet")
  for i = 1, GetNumPartyMembers() do add("party" .. i, "partypet" .. i) end
  for i = 1, GetNumRaidMembers() do add("raid" .. i, "raidpet" .. i) end
end

local function classColor(name)
  local units = { "player" }
  for i = 1, 4 do table.insert(units, "party" .. i) end
  for i = 1, 40 do table.insert(units, "raid" .. i) end
  for _, u in ipairs(units) do
    if UnitExists(u) and UnitName(u) == name then
      local _, class = UnitClass(u)
      local c = RAID_CLASS_COLORS and class and RAID_CLASS_COLORS[class]
      if c then return c.r, c.g, c.b end
      break
    end
  end
  return 0.6, 0.6, 0.6
end

-- Two segments; a fight starts with the first hit or entering combat and ends leaving combat.
local segments = {
  current = { damage = {}, total = 0, time = 0 },
  overall = { damage = {}, total = 0, time = 0 },
}
local fightStart
local view = "current"

local window = CreateFrame("Frame", "AzerothPlusMeter", UIParent)
window:SetWidth(WIDTH)
window:SetHeight(20 + BARS * (BAR_H + 1) + 4)
window:SetBackdrop({
  bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
  edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
  tile = true, tileSize = 16, edgeSize = 12,
  insets = { left = 3, right = 3, top = 3, bottom = 3 },
})
window:SetBackdropColor(0, 0, 0, 0.7)
window:SetBackdropBorderColor(0.5, 0.5, 0.5, 0.9)
window.alwaysDrag = true
AP:Movable(window, "meter", "Damage meter", "BOTTOMRIGHT", -200, 160)
window:EnableMouse(true)

local title = CreateFrame("Button", nil, window)
title:SetPoint("TOPLEFT", window, "TOPLEFT", 6, -4)
title:SetPoint("TOPRIGHT", window, "TOPRIGHT", -24, -4)
title:SetHeight(14)
title.text = title:CreateFontString(nil, "OVERLAY", "GameFontNormalSmall")
title.text:SetPoint("LEFT", title, "LEFT", 0, 0)
title:RegisterForClicks("LeftButtonUp")

local reset = CreateFrame("Button", nil, window)
reset:SetWidth(14)
reset:SetHeight(14)
reset:SetPoint("TOPRIGHT", window, "TOPRIGHT", -6, -4)
reset.text = reset:CreateFontString(nil, "OVERLAY", "GameFontNormalSmall")
reset.text:SetPoint("CENTER", reset, "CENTER", 0, 0)
reset.text:SetText("R")

local bars = {}
for i = 1, BARS do
  local bar = CreateFrame("StatusBar", nil, window)
  bar:SetStatusBarTexture("Interface\\TargetingFrame\\UI-StatusBar")
  bar:SetHeight(BAR_H)
  bar:SetPoint("TOPLEFT", window, "TOPLEFT", 5, -20 - (i - 1) * (BAR_H + 1))
  bar:SetPoint("TOPRIGHT", window, "TOPRIGHT", -5, -20 - (i - 1) * (BAR_H + 1))
  bar:SetMinMaxValues(0, 1)
  bar.left = bar:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
  bar.left:SetPoint("LEFT", bar, "LEFT", 3, 0)
  bar.right = bar:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
  bar.right:SetPoint("RIGHT", bar, "RIGHT", -3, 0)
  bar:Hide()
  bars[i] = bar
end

local function seconds(seg)
  local t = seg.time
  if fightStart then t = t + (GetTime() - fightStart) end
  return math.max(t, 1)
end

local function redraw()
  local seg = segments[view]
  title.text:SetText(view == "current" and "Damage: current fight" or "Damage: overall")
  local list = {}
  for name, amount in pairs(seg.damage) do table.insert(list, { name = name, amount = amount }) end
  table.sort(list, function(a, b) return a.amount > b.amount end)
  local top = list[1] and list[1].amount or 1
  local t = seconds(seg)
  for i = 1, BARS do
    local bar, row = bars[i], list[i]
    if row then
      bar:SetValue(row.amount / top)
      bar:SetStatusBarColor(classColor(row.name))
      bar.left:SetText(i .. ". " .. row.name)
      bar.right:SetText(AP:FormatNumber(row.amount) .. " (" .. AP:FormatNumber(row.amount / t) .. ")")
      bar:Show()
    else
      bar:Hide()
    end
  end
end

local function startFight()
  if fightStart then return end
  segments.current = { damage = {}, total = 0, time = 0 }
  fightStart = GetTime()
end

local function endFight()
  if not fightStart then return end
  local t = GetTime() - fightStart
  segments.current.time = segments.current.time + t
  segments.overall.time = segments.overall.time + t
  fightStart = nil
  redraw()
end

local function record(source, amount)
  if not amount then return end
  local owner = owners[source or UnitName("player")]
  if not owner then return end
  startFight()
  for _, seg in pairs(segments) do
    seg.damage[owner] = (seg.damage[owner] or 0) + amount
    seg.total = seg.total + amount
  end
end

local SELF_EVENTS = { "CHAT_MSG_COMBAT_SELF_HITS", "CHAT_MSG_SPELL_SELF_DAMAGE",
  "CHAT_MSG_SPELL_PERIODIC_CREATURE_DAMAGE" }
local OTHER_EVENTS = { "CHAT_MSG_COMBAT_PARTY_HITS", "CHAT_MSG_SPELL_PARTY_DAMAGE",
  "CHAT_MSG_COMBAT_PET_HITS", "CHAT_MSG_SPELL_PET_DAMAGE", "CHAT_MSG_COMBAT_FRIENDLYPLAYER_HITS",
  "CHAT_MSG_SPELL_FRIENDLYPLAYER_DAMAGE" }
local isSelf = {}
for _, e in ipairs(SELF_EVENTS) do isSelf[e] = true end

window:SetScript("OnEvent", function()
  if event == "PLAYER_REGEN_DISABLED" then
    startFight()
  elseif event == "PLAYER_REGEN_ENABLED" then
    endFight()
  elseif event == "PARTY_MEMBERS_CHANGED" or event == "RAID_ROSTER_UPDATE"
      or event == "UNIT_PET" or event == "PLAYER_ENTERING_WORLD" then
    roster()
  else
    if not selfPatterns then
      selfPatterns, otherPatterns = compile(SELF), compile(OTHER)
    end
    -- A periodic line on a creature names its caster: yours ("from your") or someone else's.
    local source, amount = parse(arg1, isSelf[event] and selfPatterns or otherPatterns)
    if not amount and event == "CHAT_MSG_SPELL_PERIODIC_CREATURE_DAMAGE" then
      source, amount = parse(arg1, otherPatterns)
    end
    record(source, amount)
  end
end)

local since = 0
window:SetScript("OnUpdate", function()
  since = since + arg1
  if since >= 0.5 then
    since = 0
    redraw()
  end
end)

title:SetScript("OnClick", function()
  view = view == "current" and "overall" or "current"
  redraw()
end)

local function resetAll()
  segments.current = { damage = {}, total = 0, time = 0 }
  segments.overall = { damage = {}, total = 0, time = 0 }
  if fightStart then fightStart = GetTime() end
  redraw()
end
reset:SetScript("OnClick", resetAll)

local function enable(on)
  local events = { "PLAYER_REGEN_DISABLED", "PLAYER_REGEN_ENABLED", "PARTY_MEMBERS_CHANGED",
    "RAID_ROSTER_UPDATE", "UNIT_PET", "PLAYER_ENTERING_WORLD", "CHAT_MSG_SPELL_PERIODIC_CREATURE_DAMAGE" }
  for _, e in ipairs(SELF_EVENTS) do table.insert(events, e) end
  for _, e in ipairs(OTHER_EVENTS) do table.insert(events, e) end
  for _, e in ipairs(events) do
    if on then window:RegisterEvent(e) else window:UnregisterEvent(e) end
  end
  if on then
    roster()
    redraw()
    window:Show()
  else
    window:Hide()
  end
end
window.refresh = function() enable(AzerothPlusDB.meter) end

AP:Register("meter", { enable = enable, reset = resetAll })

-- For tests: the parser, against the loaded GlobalStrings.
AP.meterParse = function(msg, own)
  if not selfPatterns then selfPatterns, otherPatterns = compile(SELF), compile(OTHER) end
  return parse(msg, own and selfPatterns or otherPatterns)
end
