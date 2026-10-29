-- Map & quest log: the stock world map in a window instead of over the whole screen, so you keep
-- moving and the interface stays up, with the quest log beside it: the shown zone's quests first,
-- the rest under their headings, a search, and a checkbox per quest for the objectives tracker. The
-- map's own label gives the level range of the zone under the cursor; your coordinates and the
-- cursor's sit along the bottom. Drag the top edge to move it, the corner grip to size it; the
-- button by the close button fills the screen and back.
--
-- The stock map hangs every piece from `WorldMapPositioningGuide` (1024x768, centered on a frame
-- covering the screen); the window sizes the frame to that guide. The quest panel and the labels
-- undo the map's scale, so they stay at the interface's size on a small map.

local AP = AzerothPlus

local QUESTS_WIDTH = 340
local MIN_SCALE, MAX_SCALE = 0.35, 1
local MAX_QUESTS = 20

-- Level ranges by the names the map uses (English client).
local LEVELS = {
  ["Dun Morogh"] = "1-10", ["Elwynn Forest"] = "1-10", ["Tirisfal Glades"] = "1-10",
  ["Durotar"] = "1-10", ["Mulgore"] = "1-10", ["Teldrassil"] = "1-10",
  ["Loch Modan"] = "10-20", ["Westfall"] = "10-20", ["Silverpine Forest"] = "10-20",
  ["Darkshore"] = "10-20", ["The Barrens"] = "10-25", ["Redridge Mountains"] = "15-25",
  ["Stonetalon Mountains"] = "15-27", ["Ashenvale"] = "18-30", ["Duskwood"] = "18-30",
  ["Hillsbrad Foothills"] = "20-30", ["Wetlands"] = "20-30", ["Thousand Needles"] = "25-35",
  ["Alterac Mountains"] = "30-40", ["Arathi Highlands"] = "30-40", ["Desolace"] = "30-40",
  ["Stranglethorn Vale"] = "30-45", ["Badlands"] = "35-45", ["Dustwallow Marsh"] = "35-45",
  ["Swamp of Sorrows"] = "35-45", ["Feralas"] = "40-50", ["The Hinterlands"] = "40-50",
  ["Tanaris"] = "40-50", ["Searing Gorge"] = "43-50", ["Azshara"] = "45-55",
  ["Blasted Lands"] = "45-55", ["Felwood"] = "48-55", ["Un'Goro Crater"] = "48-55",
  ["Burning Steppes"] = "50-58", ["Western Plaguelands"] = "51-58",
  ["Eastern Plaguelands"] = "53-60", ["Winterspring"] = "55-60", ["Deadwind Pass"] = "55-60",
  ["Silithus"] = "55-60",
}

-- "Loch Modan" -> "10-20" in a color for how its middle compares to your level.
local function levelText(zone)
  local levels = zone and LEVELS[zone]
  if not levels then return nil end
  local _, _, low, high = string.find(levels, "(%d+)-(%d+)")
  local c = GetDifficultyColor(math.floor((tonumber(low) + tonumber(high)) / 2))
  return string.format("|cff%02x%02x%02x%s|r", c.r * 255, c.g * 255, c.b * 255, levels)
end

local on = false
local stock -- the frame's anchors and strata and its title, read on the first switch

-- The shown map's zone name; nil on a continent or the world.
local function mapZone()
  local zone = GetCurrentMapZone()
  if zone == 0 then return nil end
  local zones = { GetMapZones(GetCurrentMapContinent()) }
  return zones[zone]
end

-- The panel right of the map.
local panel = CreateFrame("Frame", "AzerothPlusMapQuests", WorldMapFrame)
panel:SetWidth(QUESTS_WIDTH)
panel:SetBackdrop({
  bgFile = "Interface\\ChatFrame\\ChatFrameBackground",
  edgeFile = "Interface\\DialogFrame\\UI-DialogBox-Border",
  tile = false, edgeSize = 24,
  insets = { left = 6, right = 6, top = 6, bottom = 6 },
})
panel:SetBackdropColor(0.05, 0.04, 0.03, 0.92)
panel:EnableMouse(true)
panel:EnableMouseWheel(true)
panel:RegisterForDrag("LeftButton")
panel:Hide()

local zoneTitle = panel:CreateFontString(nil, "OVERLAY", "GameFontNormalLarge")
zoneTitle:SetPoint("TOPLEFT", panel, "TOPLEFT", 14, -14)
zoneTitle:SetWidth(QUESTS_WIDTH - 90)
zoneTitle:SetJustifyH("LEFT")

local close = CreateFrame("Button", nil, panel, "UIPanelCloseButton")
close:SetPoint("TOPRIGHT", panel, "TOPRIGHT", -2, -2)
close:SetScript("OnClick", function() HideUIPanel(WorldMapFrame) end)

local big = CreateFrame("Button", nil, panel)
big:SetWidth(26)
big:SetHeight(26)
big:SetPoint("RIGHT", close, "LEFT", 4, 0)
big:SetHighlightTexture("Interface\\Buttons\\UI-Common-MouseHilight", "ADD")

local search = CreateFrame("EditBox", "AzerothPlusMapSearch", panel, "InputBoxTemplate")
search:SetWidth(QUESTS_WIDTH - 130)
search:SetHeight(20)
search:SetPoint("TOPLEFT", panel, "TOPLEFT", 20, -40)
search:SetAutoFocus(false)
search:SetScript("OnEscapePressed", function() this:ClearFocus() end)
search:SetScript("OnEnterPressed", function() this:ClearFocus() end)
local searchHint = search:CreateFontString(nil, "OVERLAY", "GameFontDisableSmall")
searchHint:SetPoint("LEFT", search, "LEFT", 2, 0)
searchHint:SetText("Search quest log")

local count = panel:CreateFontString(nil, "OVERLAY", "GameFontNormal")
count:SetPoint("LEFT", search, "RIGHT", 10, 0)

local offset = 1 -- the first entry shown; the mouse wheel scrolls
local updateQuests

local list = AP:QuestList(panel, QUESTS_WIDTH - 30, {
  onFold = function(heading)
    AzerothPlusDB.mapFolded[heading] = not AzerothPlusDB.mapFolded[heading] or nil
    updateQuests()
  end,
  onCheck = function(key, tracked)
    AzerothPlusDB.untracked[key] = (not tracked) or nil
    if AP.UpdateObjectives then AP.UpdateObjectives() end
  end,
  moreText = function() return "Scroll for more" end,
})

-- Entries for the list: headings, the shown zone's first, each followed (unless folded) by its
-- quests that match the search.
local function entries(zone, filter)
  local all, order, byZone = AP:Quests(true), {}, {}
  for i = 1, table.getn(all) do
    local q = all[i]
    local text = string.lower(q.title .. "\n" .. q.body)
    if filter == "" or string.find(text, filter, 1, true) then
      if not byZone[q.zone] then
        byZone[q.zone] = {}
        if q.zone == zone then table.insert(order, 1, q.zone) else table.insert(order, q.zone) end
      end
      table.insert(byZone[q.zone], q)
    end
  end
  local out = {}
  for i = 1, table.getn(order) do
    local heading = order[i]
    local folded = AzerothPlusDB.mapFolded[heading]
    table.insert(out, { heading = heading, folded = folded })
    if not folded then
      for j = 1, table.getn(byZone[heading]) do table.insert(out, byZone[heading][j]) end
    end
  end
  return out, table.getn(all)
end

function updateQuests()
  if not on or not WorldMapFrame:IsShown() then return end
  local zone = mapZone()
  local levels = levelText(zone)
  zoneTitle:SetText((zone or GetZoneText()) .. (levels and ("  " .. levels) or ""))
  local filter = string.lower(search:GetText() or "")
  if filter == "" then searchHint:Show() else searchHint:Hide() end
  local rows, total = entries(zone, filter)
  count:SetText("Quests: |cffffffff" .. total .. "/" .. MAX_QUESTS .. "|r")
  if offset > table.getn(rows) then offset = math.max(1, table.getn(rows)) end
  local maxLines = math.floor((panel:GetHeight() - 90) / 13)
  list:Fill(rows, panel, 14, -70, maxLines, filter == "" and "No quests in the log." or "No match.", offset)
end

search:SetScript("OnTextChanged", function()
  offset = 1
  updateQuests()
end)

panel:SetScript("OnMouseWheel", function()
  offset = offset - arg1
  if offset < 1 then offset = 1 end
  updateQuests()
end)

-- Your coordinates and the cursor's along the bottom of the map.
local info = CreateFrame("Frame", nil, WorldMapFrame)
info:SetAllPoints(WorldMapDetailFrame)
info:SetFrameLevel(WorldMapDetailFrame:GetFrameLevel() + 5)
info:Hide()
local playerText = info:CreateFontString(nil, "OVERLAY", "GameFontNormal")
playerText:SetPoint("BOTTOMLEFT", WorldMapDetailFrame, "BOTTOMLEFT", 10, 8)
local cursorText = info:CreateFontString(nil, "OVERLAY", "GameFontNormal")
cursorText:SetPoint("BOTTOMRIGHT", WorldMapDetailFrame, "BOTTOMRIGHT", -10, 8)

-- The stock update finds the area under the cursor and labels it; this adds its levels and the
-- coordinates. WorldMapButton's OnUpdate calls the global, so replacing it is enough.
local stockButtonUpdate = WorldMapButton_OnUpdate
function WorldMapButton_OnUpdate(elapsed)
  stockButtonUpdate(elapsed)
  if not on then return end
  local name = WorldMapFrame.areaName
  local levels = levelText(name)
  if levels and not WorldMapFrame.poiHighlight then
    WorldMapFrameAreaLabel:SetText(name .. " (" .. levels .. ")")
  end
  local px, py = GetPlayerMapPosition("player")
  playerText:SetText((px == 0 and py == 0) and "" or string.format("Player: %.1f, %.1f", px * 100, py * 100))
  local x, y = GetCursorPosition()
  local s = this:GetEffectiveScale()
  local cx = (x / s - this:GetLeft()) / this:GetWidth()
  local cy = (this:GetTop() - y / s) / this:GetHeight()
  if cx >= 0 and cx <= 1 and cy >= 0 and cy <= 1 then
    cursorText:SetText(string.format("Cursor: %.1f, %.1f", cx * 100, cy * 100))
  else
    cursorText:SetText("")
  end
end

-- Moving: a strip along the top of the map, or the panel's top. Sizing: the panel's corner grip.
local drag = CreateFrame("Button", nil, WorldMapFrame)
drag:SetPoint("TOPLEFT", WorldMapPositioningGuide, "TOPLEFT", 0, 0)
drag:SetPoint("TOPRIGHT", WorldMapPositioningGuide, "TOPRIGHT", -40, 0)
drag:SetHeight(24)
drag:SetFrameLevel(WorldMapDetailFrame:GetFrameLevel() + 6)
drag:RegisterForDrag("LeftButton")
drag:Hide()

-- `left` and `top` are in screen units at scale 1, so a new scale keeps the window's corner put.
-- The panel and labels undo the map's scale to sit at the interface's.
local function place(left, top, scale)
  WorldMapFrame:SetScale(scale)
  WorldMapFrame:ClearAllPoints()
  WorldMapFrame:SetPoint("TOPLEFT", UIParent, "BOTTOMLEFT", left / scale, top / scale)
  local ui = UIParent:GetEffectiveScale()
  panel:SetScale(ui / scale)
  info:SetScale(ui / scale)
  -- Against the map's right border, as tall as it; offsets are in the panel's units.
  panel:ClearAllPoints()
  panel:SetPoint("TOPLEFT", WorldMapFrame, "TOPRIGHT", -6 * scale / ui, -2 * scale / ui)
  panel:SetPoint("BOTTOMLEFT", WorldMapFrame, "BOTTOMRIGHT", -6 * scale / ui, 18 * scale / ui)
end

local function panelWidth()
  return QUESTS_WIDTH * UIParent:GetEffectiveScale()
end

local function screen()
  return UIParent:GetWidth() * UIParent:GetScale(), UIParent:GetHeight() * UIParent:GetScale()
end

-- Filling the screen: the largest map that fits beside the panel, centered.
local function bigPlace()
  local w, h = screen()
  local scale = math.min((w - panelWidth() - 20) / 1024, (h - 20) / 768, MAX_SCALE)
  return (w - 1024 * scale - panelWidth()) / 2, h - (h - 768 * scale) / 2, scale
end

local function savePlace()
  local scale = WorldMapFrame:GetScale()
  AzerothPlusDB.mapPlace = { WorldMapFrame:GetLeft() * scale, WorldMapFrame:GetTop() * scale, scale }
  AzerothPlusDB.mapBig = nil
end

local function bigTextures()
  local dir = AzerothPlusDB.mapBig and "Down" or "Up"
  big:SetNormalTexture("Interface\\ChatFrame\\UI-ChatIcon-Scroll" .. dir .. "-Up")
  big:SetPushedTexture("Interface\\ChatFrame\\UI-ChatIcon-Scroll" .. dir .. "-Down")
end

local function windowed()
  local saved = AzerothPlusDB.mapPlace
  if not saved then
    local w, h = screen()
    local scale = 0.6
    saved = { (w - 1024 * scale - panelWidth()) / 2, h - 80, scale }
    AzerothPlusDB.mapPlace = saved
  end
  BlackoutWorld:Hide()
  WorldMapFrame:SetFrameStrata("MEDIUM")
  WorldMapFrame:SetWidth(1024)
  WorldMapFrame:SetHeight(768)
  if AzerothPlusDB.mapBig then
    local l, t, s = bigPlace()
    place(l, t, s)
  else
    place(saved[1], saved[2], saved[3])
  end
  bigTextures()
end

local function startMove()
  if not AzerothPlusDB.mapBig then WorldMapFrame:StartMoving() end
end

local function stopMove()
  WorldMapFrame:StopMovingOrSizing()
  if AzerothPlusDB.mapBig then return end
  savePlace()
  place(AzerothPlusDB.mapPlace[1], AzerothPlusDB.mapPlace[2], AzerothPlusDB.mapPlace[3])
end

drag:SetScript("OnDragStart", startMove)
drag:SetScript("OnDragStop", stopMove)
panel:SetScript("OnDragStart", startMove)
panel:SetScript("OnDragStop", stopMove)

big:SetScript("OnClick", function()
  AzerothPlusDB.mapBig = not AzerothPlusDB.mapBig or nil
  windowed()
  updateQuests()
end)

local grip = CreateFrame("Button", nil, panel)
grip:SetWidth(16)
grip:SetHeight(16)
grip:SetPoint("BOTTOMRIGHT", panel, "BOTTOMRIGHT", -6, 6)
grip:SetNormalTexture("Interface\\ChatFrame\\UI-ChatIM-SizeGrabber-Up")
grip:SetHighlightTexture("Interface\\ChatFrame\\UI-ChatIM-SizeGrabber-Highlight")
grip:SetPushedTexture("Interface\\ChatFrame\\UI-ChatIM-SizeGrabber-Down")
grip:SetScript("OnMouseDown", function()
  savePlace()
  bigTextures()
  this.sizing = true
end)
grip:SetScript("OnMouseUp", function()
  this.sizing = nil
  savePlace()
  updateQuests()
end)
grip:SetScript("OnUpdate", function()
  if not this.sizing then return end
  local p = AzerothPlusDB.mapPlace
  local cursor = GetCursorPosition() / UIParent:GetEffectiveScale() * UIParent:GetScale()
  local scale = (cursor - p[1] - panelWidth()) / 1024
  if scale < MIN_SCALE then scale = MIN_SCALE elseif scale > MAX_SCALE then scale = MAX_SCALE end
  place(p[1], p[2], scale)
end)

-- The stock frame's "World Map" title, found by its text.
local function titleString()
  local regions = { WorldMapFrame:GetRegions() }
  for i = 1, table.getn(regions) do
    local r = regions[i]
    if r.GetText and r:GetText() == WORLD_MAP then return r end
  end
end

local stockOnShow = WorldMapFrame:GetScript("OnShow")
WorldMapFrame:SetScript("OnShow", function()
  stockOnShow()
  if on then
    windowed()
    offset = 1
    updateQuests()
  end
end)

local events = CreateFrame("Frame")
events:SetScript("OnEvent", updateQuests)

-- With no UIPanelWindows entry, ShowUIPanel just shows the map and the interface stays up.
local stockPanel = UIPanelWindows["WorldMapFrame"]

local function setSpecial(add)
  for i = table.getn(UISpecialFrames), 1, -1 do
    if UISpecialFrames[i] == "WorldMapFrame" then table.remove(UISpecialFrames, i) end
  end
  if add then table.insert(UISpecialFrames, "WorldMapFrame") end
end

AP:Register("map", {
  enable = function(enabled)
    if enabled == on then return end
    on = enabled
    if WorldMapFrame:IsShown() then HideUIPanel(WorldMapFrame) end
    if not stock then
      stock = { strata = WorldMapFrame:GetFrameStrata(), points = {}, title = titleString() }
      for i = 1, WorldMapFrame:GetNumPoints() do
        stock.points[i] = { WorldMapFrame:GetPoint(i) }
      end
    end
    if enabled then
      UIPanelWindows["WorldMapFrame"] = nil
      setSpecial(true)
      WorldMapFrame:EnableKeyboard(false)
      WorldMapFrame:SetMovable(true)
      WorldMapFrame:SetClampedToScreen(true)
      WorldMapFrameCloseButton:Hide()
      if stock.title then stock.title:SetText("Map & Quest Log") end
      panel:Show()
      info:Show()
      drag:Show()
      events:RegisterEvent("QUEST_LOG_UPDATE")
      events:RegisterEvent("WORLD_MAP_UPDATE")
    else
      UIPanelWindows["WorldMapFrame"] = stockPanel
      setSpecial(false)
      WorldMapFrame:EnableKeyboard(true)
      WorldMapFrameCloseButton:Show()
      if stock.title then stock.title:SetText(WORLD_MAP) end
      panel:Hide()
      info:Hide()
      drag:Hide()
      events:UnregisterAllEvents()
      BlackoutWorld:Show()
      WorldMapFrame:SetFrameStrata(stock.strata)
      WorldMapFrame:ClearAllPoints()
      for i = 1, table.getn(stock.points) do
        local p = stock.points[i]
        WorldMapFrame:SetPoint(p[1], p[2], p[3], p[4], p[5])
      end
    end
  end,
})
