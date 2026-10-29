-- The quest log read for the objectives tracker and the map's quest panel, and the list both draw:
-- headings as bars that fold, quests with a difficulty-colored title, their objectives, and on the
-- map a checkbox choosing what the tracker shows.

local AP = AzerothPlus

-- A quest's objective lines; with none to count (deliver this, speak to her), its summary.
function AP:QuestObjectives(index, complete)
  local n = GetNumQuestLeaderBoards(index)
  local lines = {}
  for j = 1, n do
    local text, _, done = GetQuestLogLeaderBoard(j, index)
    if text then
      if done then text = "|cff999999" .. text .. "|r" end
      table.insert(lines, "- " .. text)
    end
  end
  if table.getn(lines) == 0 then
    local selected = GetQuestLogSelection()
    SelectQuestLogEntry(index)
    local _, summary = GetQuestLogQuestText()
    SelectQuestLogEntry(selected)
    if summary and summary ~= "" then
      table.insert(lines, "- " .. summary)
    elseif complete then
      table.insert(lines, "- Ready for turn-in")
    end
  end
  return table.concat(lines, "\n")
end

-- The log's quests in its order, each with the heading it sits under. `levels` prefixes the level.
-- A quest is keyed by its title: 1.12 gives no quest ids.
function AP:Quests(levels)
  local list, current = {}, nil
  local entries = GetNumQuestLogEntries()
  for index = 1, entries do
    local title, level, tag, isHeader, _, complete = GetQuestLogTitle(index)
    if isHeader then
      current = title
    elseif title then
      local text = title
      if levels then text = "[" .. level .. "] " .. text end
      local done = complete == 1
      table.insert(list, {
        index = index, key = title, title = text, tag = tag ~= "" and tag or nil,
        color = GetDifficultyColor(level), done = done, zone = current or "",
        body = AP:QuestObjectives(index, done),
      })
    end
  end
  return list
end

function AP:IsTracked(key)
  return not AzerothPlusDB.untracked[key]
end

-- How many lines `text` takes at `width`, roughly: a height is only known once the text has been
-- drawn, and the list must be cut before then.
local function lines(text, width, charWidth)
  local n = 0
  for line in string.gfind(text .. "\n", "(.-)\n") do
    local plain = string.gsub(string.gsub(line, "|c%x%x%x%x%x%x%x%x", ""), "|r", "")
    n = n + 1 + math.floor(string.len(plain) * charWidth / width)
  end
  return n
end

local CHECK = 20

-- Rows under `parent`, `width` wide, chained top to bottom: each hangs from the bottom of the
-- objectives above it, so the drawn text spaces them. `opts.onFold(heading)` makes heading bars
-- clickable; `opts.onCheck(key, on)` adds the track checkbox.
function AP:QuestList(parent, width, opts)
  opts = opts or {}
  local list = { rows = {} }
  local textWidth = width - 22 - (opts.onCheck and CHECK or 0)

  local function row(i)
    if list.rows[i] then return list.rows[i] end
    local r = CreateFrame("Button", nil, parent)
    r:SetWidth(width)
    r:SetHeight(16)
    r.bar = r:CreateTexture(nil, "BACKGROUND")
    r.bar:SetAllPoints(r)
    r.bar:SetTexture(0.35, 0.25, 0.1, 0.55)
    r.icon = r:CreateTexture(nil, "ARTWORK")
    r.icon:SetWidth(14)
    r.icon:SetHeight(14)
    r.icon:SetTexture("Interface\\GossipFrame\\ActiveQuestIcon")
    r.icon:SetPoint("TOPLEFT", r, "TOPLEFT", 2, -1)
    r.title = r:CreateFontString(nil, "OVERLAY", "GameFontNormal")
    r.title:SetJustifyH("LEFT")
    r.tag = r:CreateFontString(nil, "OVERLAY", "GameFontNormalSmall")
    r.tag:SetPoint("TOPRIGHT", r, "TOPRIGHT", -(opts.onCheck and CHECK or 0) - 2, -2)
    r.fold = r:CreateFontString(nil, "OVERLAY", "GameFontNormal")
    r.fold:SetPoint("RIGHT", r, "RIGHT", -6, 0)
    r.body = r:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    r.body:SetPoint("TOPLEFT", r.title, "BOTTOMLEFT", 0, -2)
    r.body:SetWidth(textWidth)
    r.body:SetJustifyH("LEFT")
    if opts.onCheck then
      r.check = CreateFrame("CheckButton", nil, r, "UICheckButtonTemplate")
      r.check:SetWidth(CHECK)
      r.check:SetHeight(CHECK)
      r.check:SetPoint("TOPRIGHT", r, "TOPRIGHT", 0, 2)
      r.check:SetScript("OnClick", function()
        opts.onCheck(this:GetParent().key, this:GetChecked() and true or false)
      end)
    end
    r:SetScript("OnClick", function()
      if this.heading then
        if opts.onFold then opts.onFold(this.heading) end
      elseif this.index then
        ShowUIPanel(QuestLogFrame)
        QuestLog_SetSelection(this.index)
        QuestLog_Update()
      end
    end)
    r:SetScript("OnEnter", function() if not this.heading then this.title:SetTextColor(1, 1, 1) end end)
    r:SetScript("OnLeave", function() this.title:SetTextColor(this.r, this.g, this.b) end)
    list.rows[i] = r
    return r
  end

  list.more = parent:CreateFontString(nil, "OVERLAY", "GameFontDisableSmall")

  -- Rows from `x`, `y` below `anchor`'s top left, from entry `start` on, cut at about `maxLines`;
  -- returns the lines used and whether entries were left below. An entry with `heading` set is a
  -- heading bar (`folded` shows it closed).
  function list:Fill(entries, anchor, x, y, maxLines, empty, start)
    local used, shown, prev, last = 0, 0, nil, (start or 1) - 1
    for i = start or 1, table.getn(entries) do
      local e = entries[i]
      local cost = e.heading and 2 or
        (lines(e.title, textWidth - 60, 7) + lines(e.body, textWidth, 5.5))
      if shown > 0 and used + cost > maxLines then break end
      shown = shown + 1
      used = used + cost
      last = i
      local r = row(shown)
      r.index, r.key, r.heading = e.index, e.key, e.heading
      local color = e.color or QuestDifficultyColor["header"]
      r.r, r.g, r.b = color.r, color.g, color.b
      r.title:SetText(e.heading or e.title)
      r.title:SetTextColor(r.r, r.g, r.b)
      r.title:ClearAllPoints()
      if e.heading then
        r:SetHeight(18)
        r.title:SetPoint("LEFT", r, "LEFT", 6, 0)
        r.title:SetWidth(width - 30)
        r.bar:Show()
        r.icon:Hide()
        r.tag:SetText("")
        r.body:SetText("")
        r.fold:SetText(e.folded and "+" or "-")
        r.fold:Show()
        if r.check then r.check:Hide() end
      else
        r:SetHeight(16)
        r.title:SetPoint("TOPLEFT", r, "TOPLEFT", 20, 0)
        r.title:SetWidth(textWidth - (e.tag and 60 or 0))
        r.bar:Hide()
        local shade = e.done and 1 or 0.45
        r.icon:SetVertexColor(shade, shade, shade)
        r.icon:Show()
        r.tag:SetText(e.tag and ("(" .. e.tag .. ")") or "")
        r.body:SetText(e.body)
        r.fold:Hide()
        if r.check then
          r.check:SetChecked(AP:IsTracked(e.key))
          r.check:Show()
        end
      end
      r:ClearAllPoints()
      if not prev then
        r:SetPoint("TOPLEFT", anchor, "TOPLEFT", x, y)
      elseif prev.heading then
        r:SetPoint("TOPLEFT", prev, "BOTTOMLEFT", 0, -4)
      else
        r:SetPoint("TOPLEFT", prev.body, "BOTTOMLEFT", -20, e.heading and -8 or -6)
      end
      r:Show()
      prev = r
    end
    for i = shown + 1, table.getn(self.rows) do self.rows[i]:Hide() end

    local below = last < table.getn(entries)
    self.more:ClearAllPoints()
    if not prev then
      self.more:SetPoint("TOPLEFT", anchor, "TOPLEFT", x + 2, y)
    elseif prev.heading then
      self.more:SetPoint("TOPLEFT", prev, "BOTTOMLEFT", 20, -4)
    else
      self.more:SetPoint("TOPLEFT", prev.body, "BOTTOMLEFT", 0, -6)
    end
    local note = shown == 0 and empty
    if below and opts.moreText then note = opts.moreText(entries, last) end
    if note then
      self.more:SetText(note)
      self.more:Show()
      used = used + 1
    else
      self.more:Hide()
    end
    return used, below
  end

  return list
end
