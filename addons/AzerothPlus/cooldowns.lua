-- Cooldown manager: one movable display of what is cooling down (spells, trinkets, bag items such as
-- potions) and, below it, the player's own timed buffs, each with its time left.

local AP = AzerothPlus

local SIZE, GAP, PER_ROW = 34, 4, 10
local GCD = 1.6 -- a cooldown this short is the global cooldown, not the spell's own
local MAX_BUFF = 600 -- buffs longer than this (10 min) are long-term ones, not worth a slot

local frame = CreateFrame("Frame", "AzerothPlusCooldowns", UIParent)
frame:SetWidth(PER_ROW * (SIZE + GAP) - GAP)
frame:SetHeight(SIZE * 2 + GAP + 12)
AP:Movable(frame, "cooldowns", "Cooldowns", "CENTER", 0, -190)

local function newIcon(parent)
  local b = CreateFrame("Frame", nil, parent)
  b:SetWidth(SIZE)
  b:SetHeight(SIZE)
  b.icon = b:CreateTexture(nil, "ARTWORK")
  b.icon:SetAllPoints(b)
  b.icon:SetTexCoord(0.07, 0.93, 0.07, 0.93)
  b.shade = b:CreateTexture(nil, "OVERLAY")
  b.shade:SetAllPoints(b)
  b.shade:SetTexture(0, 0, 0, 0.35)
  b.time = b:CreateFontString(nil, "OVERLAY", "NumberFontNormal")
  b.time:SetPoint("CENTER", b, "CENTER", 0, 0)
  b.count = b:CreateFontString(nil, "OVERLAY", "NumberFontNormalSmall")
  b.count:SetPoint("BOTTOMRIGHT", b, "BOTTOMRIGHT", -1, 2)
  return b
end

local rows = { cd = { icons = {}, top = 0 }, buff = { icons = {}, top = -(SIZE + GAP + 12) } }

local function icon(row, i)
  local r = rows[row]
  local b = r.icons[i]
  if not b then
    b = newIcon(frame)
    b:SetPoint("TOPLEFT", frame, "TOPLEFT", (i - 1) * (SIZE + GAP), r.top)
    r.icons[i] = b
  end
  return b
end

-- What is cooling down, longest-known first: {texture, ends, duration}, one per texture.
local function cooldowns()
  local list, seen, now = {}, {}, GetTime()
  local function add(texture, start, duration)
    if texture and start and start > 0 and duration and duration > GCD and not seen[texture] then
      seen[texture] = true
      table.insert(list, { texture = texture, ends = start + duration })
    end
  end
  for tab = 1, GetNumSpellTabs() do
    local _, _, offset, count = GetSpellTabInfo(tab)
    for i = offset + 1, offset + count do
      local start, duration = GetSpellCooldown(i, BOOKTYPE_SPELL)
      add(GetSpellTexture(i, BOOKTYPE_SPELL), start, duration)
    end
  end
  for _, slot in ipairs({ 13, 14 }) do
    local start, duration = GetInventoryItemCooldown("player", slot)
    add(GetInventoryItemTexture("player", slot), start, duration)
  end
  for bag = 0, 4 do
    for slot = 1, GetContainerNumSlots(bag) do
      local start, duration = GetContainerItemCooldown(bag, slot)
      if start and start > 0 then
        add(GetContainerItemInfo(bag, slot), start, duration)
      end
    end
  end
  table.sort(list, function(a, b) return a.ends < b.ends end)
  for i = table.getn(list), 1, -1 do
    if list[i].ends <= now then
      table.remove(list, i)
    end
  end
  return list
end

-- The player's own timed buffs, soonest to end first.
local function buffs()
  local list = {}
  for i = 0, 31 do
    local index = GetPlayerBuff(i, "HELPFUL")
    if index < 0 then
      break
    end
    local left = GetPlayerBuffTimeLeft(index)
    if left and left > 0 and left <= MAX_BUFF then
      table.insert(list, {
        texture = GetPlayerBuffTexture(index),
        ends = GetTime() + left,
        count = GetPlayerBuffApplications(index),
      })
    end
  end
  table.sort(list, function(a, b) return a.ends < b.ends end)
  return list
end

local shown = { cd = {}, buff = {} }

local function fill(row, list)
  local n = math.min(table.getn(list), PER_ROW)
  for i = 1, n do
    local b = icon(row, i)
    b.icon:SetTexture(list[i].texture)
    b.ends = list[i].ends
    local c = list[i].count
    b.count:SetText(c and c > 1 and c or "")
    b:Show()
  end
  for i = n + 1, table.getn(rows[row].icons) do
    rows[row].icons[i]:Hide()
  end
  shown[row] = n
end

local dirty, elapsedSince = true, 0
local function rebuild()
  fill("cd", AzerothPlusDB.cooldowns and cooldowns() or {})
  fill("buff", AzerothPlusDB.buffs and buffs() or {})
  dirty = false
end

local function tick()
  elapsedSince = elapsedSince + arg1
  if elapsedSince < 0.1 then
    return
  end
  elapsedSince = 0
  if dirty then
    rebuild()
  end
  local now = GetTime()
  for _, row in ipairs({ "cd", "buff" }) do
    for i = 1, shown[row] do
      local b = rows[row].icons[i]
      local left = b.ends - now
      if left <= 0 then
        dirty = true
      else
        b.time:SetText(AP:FormatTime(left))
      end
    end
  end
end

frame:SetScript("OnEvent", function() dirty = true end)

local function enable()
  local on = AzerothPlusDB.cooldowns or AzerothPlusDB.buffs
  local events = { "SPELL_UPDATE_COOLDOWN", "BAG_UPDATE_COOLDOWN", "ACTIONBAR_UPDATE_COOLDOWN",
    "PLAYER_AURAS_CHANGED", "PLAYER_ENTERING_WORLD" }
  for _, e in ipairs(events) do
    if on then frame:RegisterEvent(e) else frame:UnregisterEvent(e) end
  end
  if on then
    dirty = true
    frame:SetScript("OnUpdate", tick)
    frame:Show()
  else
    frame:SetScript("OnUpdate", nil)
    if AzerothPlusDB.locked then frame:Hide() end
  end
end
frame.refresh = enable

AP:Register("cooldowns", { enable = enable })
AP:Register("buffs", { enable = enable })
