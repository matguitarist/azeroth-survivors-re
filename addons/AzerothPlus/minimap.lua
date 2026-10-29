-- Minimap: your coordinates under it, and the buff and weapon-enchant icons moved off its left
-- edge, where the stock layout packs them against it; "Move Frames" drags them anywhere.

local AP = AzerothPlus

local coords = CreateFrame("Frame", "AzerothPlusCoords", MinimapCluster)
coords:SetWidth(70)
coords:SetHeight(16)
coords:SetPoint("TOP", Minimap, "BOTTOM", 0, -4)
coords:SetBackdrop({ bgFile = "Interface\\ChatFrame\\ChatFrameBackground" })
coords:SetBackdropColor(0, 0, 0, 0.5)
coords:Hide()
local text = coords:CreateFontString(nil, "OVERLAY", "GameFontNormalSmall")
text:SetPoint("CENTER", coords, "CENTER", 0, 0)

-- GetPlayerMapPosition reads the map's current zone, so point it back at where you are whenever
-- you move between zones or close the map on another one.
local function here()
  if not WorldMapFrame:IsShown() then SetMapToCurrentZone() end
end

local elapsedSince = 0
coords:SetScript("OnUpdate", function()
  elapsedSince = elapsedSince + arg1
  if elapsedSince < 0.2 then return end
  elapsedSince = 0
  local x, y = GetPlayerMapPosition("player")
  if x == 0 and y == 0 then
    text:SetText("")
    this:SetAlpha(0)
  else
    text:SetText(string.format("%.1f, %.1f", x * 100, y * 100))
    this:SetAlpha(1)
  end
end)
coords:SetScript("OnEvent", here)

local stockHide = WorldMapFrame:GetScript("OnHide")
WorldMapFrame:SetScript("OnHide", function()
  if stockHide then stockHide() end
  if AzerothPlusDB.minimap then SetMapToCurrentZone() end
end)

-- The buffs hang from the weapon-enchant frame (BuffFrame.lua re-anchors them as enchants come
-- and go), so that one hangs from this anchor and both follow it. The anchor's top right is the
-- first icon's; the rows grow left and down from there.
local buffs = CreateFrame("Frame", "AzerothPlusBuffAnchor", UIParent)
buffs:SetWidth(280)
buffs:SetHeight(80)
AP:Movable(buffs, "buffIcons", "Buffs", "TOPRIGHT", -230, -13)
buffs.refresh = function()
  if not AzerothPlusDB.minimap then buffs:Hide() end
end

local stockEnchant -- TemporaryEnchantFrame's own anchor, kept to put back

local function remember()
  local p, rel, rp, x, y = TemporaryEnchantFrame:GetPoint()
  stockEnchant = { p, rel or UIParent, rp, x, y }
end

local function pin()
  TemporaryEnchantFrame:ClearAllPoints()
  TemporaryEnchantFrame:SetPoint("TOPRIGHT", buffs, "TOPRIGHT", 0, 0)
end

-- The GM ticket status puts the enchant frame back on every world entry (under the ticket box
-- while one is open): that spot becomes the one to restore, and the frame goes back on the anchor.
local stockTicket = TicketStatusFrame:GetScript("OnEvent")
TicketStatusFrame:SetScript("OnEvent", function()
  if stockTicket then stockTicket() end
  if AzerothPlusDB.minimap then
    remember()
    pin()
  end
end)

local function moveBuffs(on)
  if on then
    if not stockEnchant then remember() end
    buffs:Show()
    pin()
  elseif stockEnchant then
    TemporaryEnchantFrame:ClearAllPoints()
    TemporaryEnchantFrame:SetPoint(stockEnchant[1], stockEnchant[2], stockEnchant[3], stockEnchant[4],
      stockEnchant[5])
    if AzerothPlusDB.locked then buffs:Hide() end
  end
end

AP:Register("minimap", {
  enable = function(on)
    moveBuffs(on)
    if on then
      for _, e in ipairs({ "PLAYER_ENTERING_WORLD", "ZONE_CHANGED_NEW_AREA", "ZONE_CHANGED",
        "ZONE_CHANGED_INDOORS" }) do
        coords:RegisterEvent(e)
      end
      here()
      coords:Show()
    else
      coords:UnregisterAllEvents()
      coords:Hide()
    end
  end,
})
