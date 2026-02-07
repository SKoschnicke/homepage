-- Pandoc custom writer for Gemini format
-- Converts Markdown AST to text/gemini output
--
-- Gemini format:
--   # Heading 1 / ## Heading 2 / ### Heading 3
--   => URL Link text
--   * List item
--   > Quote (some clients)
--   ``` preformatted block ```
--   Plain paragraphs separated by blank lines

-- Track collected links to output after paragraph
local collected_links = {}

-- Helper to stringify inline elements (strips all formatting)
local function stringify(inlines)
    local result = {}
    for _, inline in ipairs(inlines) do
        if inline.t == "Str" then
            table.insert(result, inline.text)
        elseif inline.t == "Space" then
            table.insert(result, " ")
        elseif inline.t == "SoftBreak" then
            table.insert(result, " ")
        elseif inline.t == "LineBreak" then
            table.insert(result, "\n")
        elseif inline.t == "Code" then
            -- Inline code: just output the text, no backticks
            table.insert(result, inline.text)
        elseif inline.t == "Emph" or inline.t == "Strong" or
               inline.t == "Strikeout" or inline.t == "Superscript" or
               inline.t == "Subscript" or inline.t == "SmallCaps" then
            -- Strip formatting markers, keep content
            table.insert(result, stringify(inline.content))
        elseif inline.t == "Link" then
            -- Collect link for output after paragraph
            local text = stringify(inline.content)
            local url = inline.target
            table.insert(collected_links, {url = url, text = text})
            table.insert(result, text)
        elseif inline.t == "Image" then
            -- Collect image as link
            local alt = stringify(inline.caption)
            local url = inline.src
            if alt == "" then
                alt = "Image"
            end
            table.insert(collected_links, {url = url, text = alt})
            table.insert(result, "[" .. alt .. "]")
        elseif inline.t == "Quoted" then
            local q = inline.quotetype == "SingleQuote" and "'" or '"'
            table.insert(result, q .. stringify(inline.content) .. q)
        elseif inline.t == "RawInline" then
            -- Handle raw HTML like <br />
            if inline.format == "html" then
                if inline.text:match("^%s*<br%s*/?%s*>%s*$") then
                    table.insert(result, "\n")
                end
                -- Ignore other HTML
            else
                table.insert(result, inline.text)
            end
        elseif inline.t == "Math" then
            -- Math: just output the raw text
            table.insert(result, inline.text)
        elseif inline.t == "Note" then
            -- Footnotes: skip for Gemini
        elseif inline.t == "Span" then
            table.insert(result, stringify(inline.content))
        end
    end
    return table.concat(result)
end

-- Output collected links and clear the list
local function flush_links()
    local result = {}
    for _, link in ipairs(collected_links) do
        table.insert(result, "=> " .. link.url .. " " .. link.text)
    end
    collected_links = {}
    if #result > 0 then
        return "\n" .. table.concat(result, "\n")
    end
    return ""
end

-- Block element handlers
function Writer(doc, opts)
    local buffer = {}

    local function add(s)
        table.insert(buffer, s)
    end

    for _, block in ipairs(doc.blocks) do
        if block.t == "Para" then
            local text = stringify(block.content)
            add(text)
            add(flush_links())
            add("")  -- blank line after paragraph

        elseif block.t == "Plain" then
            local text = stringify(block.content)
            add(text)
            add(flush_links())

        elseif block.t == "Header" then
            local level = block.level
            local prefix = string.rep("#", math.min(level, 3)) .. " "
            add(prefix .. stringify(block.content))
            add("")

        elseif block.t == "CodeBlock" then
            -- Code blocks use ``` delimiters
            add("```")
            add(block.text)
            add("```")
            add("")

        elseif block.t == "BlockQuote" then
            -- Blockquotes: prefix each line with >
            local inner = {}
            for _, b in ipairs(block.content) do
                if b.t == "Para" or b.t == "Plain" then
                    local text = stringify(b.content)
                    for line in text:gmatch("[^\n]+") do
                        table.insert(inner, "> " .. line)
                    end
                end
            end
            add(table.concat(inner, "\n"))
            add(flush_links())
            add("")

        elseif block.t == "BulletList" then
            for _, item in ipairs(block.content) do
                local lines = {}
                for _, b in ipairs(item) do
                    if b.t == "Para" or b.t == "Plain" then
                        table.insert(lines, stringify(b.content))
                    end
                end
                add("* " .. table.concat(lines, " "))
                add(flush_links())
            end
            add("")

        elseif block.t == "OrderedList" then
            local num = block.start or 1
            for _, item in ipairs(block.content) do
                local lines = {}
                for _, b in ipairs(item) do
                    if b.t == "Para" or b.t == "Plain" then
                        table.insert(lines, stringify(b.content))
                    end
                end
                -- Gemini doesn't have ordered lists, use * with number
                add("* " .. num .. ". " .. table.concat(lines, " "))
                add(flush_links())
                num = num + 1
            end
            add("")

        elseif block.t == "HorizontalRule" then
            add("---")
            add("")

        elseif block.t == "RawBlock" then
            if block.format == "html" then
                -- Skip HTML blocks entirely
            else
                add(block.text)
                add("")
            end

        elseif block.t == "Div" then
            -- Process div contents
            for _, b in ipairs(block.content) do
                -- Recursively handle
            end

        elseif block.t == "DefinitionList" then
            for _, item in ipairs(block.content) do
                local term = stringify(item[1])
                add(term)
                for _, def in ipairs(item[2]) do
                    for _, b in ipairs(def) do
                        if b.t == "Para" or b.t == "Plain" then
                            add("  " .. stringify(b.content))
                        end
                    end
                end
            end
            add("")
        end
    end

    -- Clean up: collapse multiple blank lines
    local output = table.concat(buffer, "\n")
    output = output:gsub("\n\n\n+", "\n\n")
    output = output:gsub("^\n+", "")  -- trim leading newlines
    output = output:gsub("\n+$", "")  -- trim trailing newlines

    return output .. "\n"
end
