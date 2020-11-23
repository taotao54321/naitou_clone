local util = {}

-- lhs と rhs が等しいかどうかを返す。
-- テーブルについては、メタメソッド __eq があればそれを使い、さもなくば再帰的に
-- 中身を比較する。
--
-- CAUTION: 循環参照テーブルを渡すと無限再帰する。
function util.eq(lhs, rhs)
    local ty = type(lhs)
    if ty ~= type(rhs) then return false end

    if ty ~= "table" then return lhs == rhs end
    -- lhs, rhs はテーブル

    local mt = getmetatable(lhs)
    if mt and mt.__eq then return lhs == rhs end

    -- キー集合が一致していなければ false
    for k in pairs(lhs) do
        if rhs[k] == nil then return false end
    end
    for k in pairs(rhs) do
        if lhs[k] == nil then return false end
    end

    for k, v in pairs(lhs) do
        if not util.eq(v, rhs[k]) then return false end
    end

    return true
end

-- util.eq の否定を返す。
function util.ne(lhs, rhs)
    return not util.eq(lhs, rhs)
end

-- テーブルのキーたちを未ソートで格納した配列を返す。
function util.keys(tbl)
    local res = {}
    for k in pairs(tbl) do
        table.insert(res, k)
    end
    return res
end

-- arg を human-readable な文字列に変換する。
--
-- CAUTION: 循環参照テーブルを渡すと無限再帰する。
--
-- TODO: arg がテーブルの場合、キーの順序が不定。
-- ソートしたいのだが、キー型が不揃いだとエラーになるのが面倒で保留。
function util.str(arg)
    local ty = type(arg)

    if ty == "string" then return '"' .. arg .. '"' end

    if ty ~= "table" then return tostring(arg) end
    -- arg はテーブル

    local buf = {}
    table.insert(buf, "{")
    for k, v in pairs(arg) do
        table.insert(buf, string.format(" [%s]=", util.str(k)))
        table.insert(buf, string.format("%s, ", util.str(v)))
    end
    table.insert(buf, "}")

    return table.concat(buf)
end

return util
