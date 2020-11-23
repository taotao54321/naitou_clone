local mesen = require("mesen")
local read       = mesen.read
local read_array = mesen.read_array

local naitou = {
    PT_KING       = 1,
    PT_ROOK       = 2,
    PT_BISHOP     = 3,
    PT_GOLD       = 4,
    PT_SILVER     = 5,
    PT_KNIGHT     = 6,
    PT_LANCE      = 7,
    PT_PAWN       = 8,
    PT_DRAGON     = 9,
    PT_HORSE      = 10,
    PT_PRO_SILVER = 12,
    PT_PRO_KNIGHT = 13,
    PT_PRO_LANCE  = 14,
    PT_PRO_PAWN   = 15,
}

function naitou.is_hum_turn()
    return read(0x77) ~= 0
end

function naitou.board_hum()
    return read_array(0x3A9, 11*11)
end

function naitou.board_com()
    return read_array(0x49B, 11*11)
end

function naitou.hand_hum()
    return read_array(0x58D, 7)
end

function naitou.hand_com()
    return read_array(0x594, 7)
end

function naitou.effect_board_hum()
    return read_array(0x422, 11*11)
end

function naitou.effect_board_com()
    return read_array(0x514, 11*11)
end

function naitou.position()
    return {
        is_hum_turn = naitou.is_hum_turn(),
        board_hum   = naitou.board_hum(),
        board_com   = naitou.board_com(),
        hand_hum    = naitou.hand_hum(),
        hand_com    = naitou.hand_com(),
    }
end

function naitou.handicap()
    return read(0xFE)
end

function naitou.book_state()
    return {
        formation   = read(0x5BE),
        done_branch = read_array(0x2C, 16),
        done_moves  = read_array(0x3C, 24)
    }
end

function naitou.ai_state()
    return {
        progress_ply       = read(0x5C1),
        progress_level     = read(0x28E),
        progress_level_sub = read(0x5C8),
        book_state         = naitou.book_state(),
    }
end

function naitou.move_hum()
    return {
        src          = read(0x5A2),
        dst          = read(0x5A1),
        is_promotion = read(0x5BF) ~= 0,
    }
end

function naitou.move_com()
    return {
        src          = read(0x5BC),
        dst          = read(0x5BB),
        is_promotion = read(0x5C0) ~= 0,
    }
end

function naitou.move_cand()
    return {
        src          = read(0x277),
        dst          = read(0x276),
        is_promotion = read(0x279) ~= 0,
    }
end

function naitou.move_best()
    return {
        src          = read(0x285),
        dst          = read(0x284),
        is_promotion = read(0x28C) ~= 0,
    }
end

function naitou.root_eval()
    return {
        adv_price    = read(0x280),
        disadv_price = read(0x282),
        power_com    = read(0x5E4),
        power_hum    = read(0x5E7),
        rbp_com      = read(0x5EA),
    }
end

function naitou.position_eval()
    return {
        adv_price            = read(0x272),
        adv_sq               = read(0x273),
        com_king_safety_far  = read(0x295),
        com_king_threat_far  = read(0x296),
        com_king_threat_near = read(0x5EB),
        disadv_price         = read(0x274),
        disadv_sq            = read(0x275),
        hanging_hum          = read(0x5DF),
        hum_king_threat_far  = read(0x299),
        n_choke_sq           = read(0x5E5),
        n_loose              = read(0x297),
        n_promoted_com       = read(0x293),
        n_promoted_hum       = read(0x5E8),
    }
end

function naitou.cand_eval()
    return {
        adv_price           = read(0x272),
        capture_price       = read(0x278),
        disadv_price        = read(0x274),
        dst_to_hum_king     = read(0x294),
        is_sacrifice        = read(0x27C),
        nega                = read(0x5E0),
        posi                = read(0x2A4),
        src_to_hum_king     = read(0x298),
    }
end

function naitou.best_eval()
    return {
        adv_price            = read(0x286),
        adv_sq               = read(0x287),
        capture_price        = read(0x28A),
        com_king_safety_far  = read(0x29C),
        com_king_threat_far  = read(0x29D),
        disadv_price         = read(0x288),
        disadv_sq            = read(0x289),
        dst_to_hum_king      = read(0x29B),
        hum_king_threat_far  = read(0x2A0),
        n_loose              = read(0x29E),
        n_promoted_com       = read(0x29A),
        nega                 = read(0x5E2),
        posi                 = read(0x2A6),
        src_to_hum_king      = read(0x29F),
    }
end

----------------------------------------------------------------------
-- pretty
----------------------------------------------------------------------

function naitou.pretty_sq_x(x)
    local STRS = { "１", "２", "３", "４", "５", "６", "７", "８", "９" }
    return STRS[10-x]
end

function naitou.pretty_sq_y(y)
    local STRS = { "一", "二", "三", "四", "五", "六", "七", "八", "九" }
    return STRS[y]
end

function naitou.pretty_sq(sq)
    local x, y = naitou.sq2xy(sq)
    return naitou.pretty_sq_x(x) .. naitou.pretty_sq_y(y)
end

function naitou.pretty_pt(pt)
    local STRS = { "玉", "飛", "角", "金", "銀", "桂", "香", "歩", "龍", "馬", "禁", "全", "圭", "杏", "と" }
    return STRS[pt]
end

function naitou.pretty_move_hum(move)
    if naitou.move_is_drop(move) then
        return naitou.pretty_move_drop_hum(move)
    else
        return naitou.pretty_move_nondrop_hum(move)
    end
end

function naitou.pretty_move_nondrop_hum(move)
    return string.format("%s%s%s",
        naitou.pretty_sq(move.src),
        naitou.pretty_sq(move.dst),
        move.is_promotion and "成" or ""
    )
end

function naitou.pretty_move_drop_hum(move)
    local pt = naitou.drop_move2pt_hum(move)
    return string.format("%s%s打",
        naitou.pretty_sq(move.dst),
        naitou.pretty_pt(pt)
    )
end

function naitou.pretty_move_com(move)
    if naitou.move_is_drop(move) then
        return naitou.pretty_move_drop_com(move)
    else
        return naitou.pretty_move_nondrop_com(move)
    end
end

function naitou.pretty_move_nondrop_com(move)
    return string.format("%s%s%s",
        naitou.pretty_sq(move.src),
        naitou.pretty_sq(move.dst),
        move.is_promotion and "成" or ""
    )
end

function naitou.pretty_move_drop_com(move)
    local pt = naitou.drop_move2pt_com(move)
    return string.format("%s%s打",
        naitou.pretty_sq(move.dst),
        naitou.pretty_pt(pt)
    )
end

----------------------------------------------------------------------
-- util
----------------------------------------------------------------------

function naitou.sq2xy(sq)
    local x = sq %  11
    local y = sq // 11
    return x, y
end

function naitou.xy2sq(x, y)
    return 11*y + x
end

function naitou.move_is_drop(move)
    return naitou.src_is_drop(move.src)
end

function naitou.src_is_drop(src)
    return src >= 200
end

function naitou.drop_move2pt_hum(move)
    return naitou.drop_src2pt_hum(move.src)
end

function naitou.drop_src2pt_hum(src)
    return src - 211
end

function naitou.drop_move2pt_com(move)
    return naitou.drop_src2pt_com(move.src)
end

function naitou.drop_src2pt_com(src)
    return 209 - src
end

return naitou
