-- The options window (/ap): a checkbox per feature, and the lock that lets frames be dragged.

local AP = AzerothPlus

local ROWS = {
  { "cooldowns", "Cooldown manager: spells, trinkets and potions" },
  { "buffs", "Cooldown manager: your timed buffs" },
  { "meter", "Damage meter" },
  { "swing", "Swing and Auto Shot timer" },
  { "resource", "Personal resource display" },
  { "resourceInCombatOnly", "    Only in combat" },
  { "combo", "    Combo points" },
}

local panel = CreateFrame("Frame", "AzerothPlusOptions", UIParent)
panel:SetWidth(340)
panel:SetHeight(84 + table.getn(ROWS) * 26)
panel:SetPoint("CENTER", UIParent, "CENTER", 0, 60)
panel:SetFrameStrata("DIALOG")
panel:SetBackdrop({
  bgFile = "Interface\\DialogFrame\\UI-DialogBox-Background",
  edgeFile = "Interface\\DialogFrame\\UI-DialogBox-Border",
  tile = true, tileSize = 32, edgeSize = 32,
  insets = { left = 11, right = 12, top = 12, bottom = 11 },
})
panel:EnableMouse(true)
panel:SetMovable(true)
panel:RegisterForDrag("LeftButton")
panel:SetScript("OnDragStart", function() this:StartMoving() end)
panel:SetScript("OnDragStop", function() this:StopMovingOrSizing() end)
panel:Hide()
table.insert(UISpecialFrames, "AzerothPlusOptions") -- Escape closes it

local heading = panel:CreateFontString(nil, "ARTWORK", "GameFontNormalLarge")
heading:SetPoint("TOP", panel, "TOP", 0, -18)
heading:SetText("AzerothPlus")

local close = CreateFrame("Button", nil, panel, "UIPanelCloseButton")
close:SetPoint("TOPRIGHT", panel, "TOPRIGHT", -6, -6)

local boxes = {}
for i, row in ipairs(ROWS) do
  local key = row[1]
  local box = CreateFrame("CheckButton", "AzerothPlusOption" .. i, panel, "OptionsCheckButtonTemplate")
  box:SetPoint("TOPLEFT", panel, "TOPLEFT", 20, -44 - (i - 1) * 26)
  getglobal(box:GetName() .. "Text"):SetText(row[2])
  box:SetScript("OnClick", function()
    AP:Set(key, this:GetChecked() and true or false)
  end)
  box.key = key
  boxes[i] = box
end

local lock = CreateFrame("Button", nil, panel, "UIPanelButtonTemplate")
lock:SetWidth(140)
lock:SetHeight(22)
lock:SetPoint("BOTTOMLEFT", panel, "BOTTOMLEFT", 20, 18)
local function lockText()
  lock:SetText(AzerothPlusDB.locked and "Move Frames" or "Lock Frames")
end
lock:SetScript("OnClick", function()
  AP:Set("locked", not AzerothPlusDB.locked)
  lockText()
end)

local resetButton = CreateFrame("Button", nil, panel, "UIPanelButtonTemplate")
resetButton:SetWidth(140)
resetButton:SetHeight(22)
resetButton:SetPoint("BOTTOMRIGHT", panel, "BOTTOMRIGHT", -20, 18)
resetButton:SetText("Reset Positions")
resetButton:SetScript("OnClick", function() AP:ResetPositions() end)

panel:SetScript("OnShow", function()
  for _, box in ipairs(boxes) do box:SetChecked(AzerothPlusDB[box.key]) end
  lockText()
end)

function AP:ToggleOptions()
  if panel:IsShown() then panel:Hide() else panel:Show() end
end
