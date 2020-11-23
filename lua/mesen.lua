local mesen = {}

if not (emu and emu.addEventCallback) then
    error("not mesen host")
end

function mesen.read(addr)
    return emu.read(addr, emu.memType.cpu)
end

function mesen.read_array(addr, size)
    local buf = {}
    for off = 0, size-1 do
        table.insert(buf, mesen.read(addr+off))
    end
    return buf
end

function mesen.hook_exec(f, addr)
    emu.addMemoryCallback(f, emu.memCallbackType.cpuExec, addr)
end

return mesen
