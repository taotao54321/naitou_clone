local fmt = string.format

local util = require("util")

local mesen = require("mesen")
local hook_exec = mesen.hook_exec

local naitou = require("naitou")

local function main()
    -- Think: ルート局面評価後
    hook_exec(function()
        local mv_hum = naitou.move_hum()
        print(fmt("HUM: %s", naitou.pretty_move_hum(mv_hum)))
        print(fmt("AI: %s", util.str(naitou.ai_state())))
        print(fmt("ROOT_EVAL: %s", util.str(naitou.root_eval())))
    end, 0xF03E)

    -- EvalMoveNondrop: TryImproveBest 呼び出し直後
    hook_exec(function()
        local mv_cand = naitou.move_cand()
        print(fmt("CAND: %s", naitou.pretty_move_com(mv_cand)))
        print(fmt("POS_EVAL: %s", util.str(naitou.position_eval())))
        print(fmt("CAND_EVAL: %s", util.str(naitou.cand_eval())))
    end, 0xF0F8)

    -- EvalMoveDrop: TryImproveBest 呼び出し直後
    hook_exec(function()
        local mv_cand = naitou.move_cand()
        print(fmt("CAND: %s", naitou.pretty_move_com(mv_cand)))
        print(fmt("POS_EVAL: %s", util.str(naitou.position_eval())))
        print(fmt("CAND_EVAL: %s", util.str(naitou.cand_eval())))
    end, 0xF25C)

    -- ImproveBest: 最善手更新完了
    hook_exec(function()
        local best = naitou.move_best()
        print(fmt("BEST: %s", naitou.pretty_move_com(best)))
        print(fmt("BEST_EVAL: %s", util.str(naitou.best_eval())))
    end, 0xF76B)

    -- COM 着手決定
    hook_exec(function()
        local move_com = naitou.move_com()
        print(fmt("COM: %s", naitou.pretty_move_com(move_com)))
    end, 0xDDF0)
end

main()
