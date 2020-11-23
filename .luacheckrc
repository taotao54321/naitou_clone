std = "lua53"

read_globals = {
    emu = {
        fields = {
            -- Callbacks
            addEventCallback = {},
            removeEventCallback = {},
            addMemoryCallback = {},
            removeMemoryCallback = {},

            -- Drawing
            drawPixel = {},
            drawLine = {},
            drawRectangle = {},
            drawString = {},
            clearScreen = {},
            getPixel = {},
            getScreenBuffer = {},
            setScreenBuffer = {},

            -- Emulation
            getState = {},
            setState = {},
            breakExecution = {},
            execute = {},
            reset = {},
            stop = {},
            resume = {},
            rewind = {},

            -- Input
            getInput = {},
            setInput = {},
            getMouseState = {},
            isKeyPressed = {},

            -- Logging
            displayMessage = {},
            log = {},

            -- Memory Access
            read = {},
            readWord = {},
            write = {},
            writeWord = {},
            revertPrgChrChanges = {},
            getPrgRomOffset = {},
            getChrRomOffset = {},
            getLabelAddress = {},

            -- Miscellaneous
            saveSavestate = {},
            saveSavestateAsync = {},
            loadSavestate = {},
            loadSavestateAsync = {},
            getSavestateData = {},
            clearSavestateData = {},
            addCheat = {},
            clearCheats = {},
            getAccessCounters = {},
            resetAccessCounters = {},
            getLogWindowLog = {},
            getRomInfo = {},
            getScriptDataFolder = {},
            takeScreenshot = {},

            -- Enums
            eventType = {
                fields = {
                    reset = {},
                    nmi = {},
                    irq = {},
                    startFrame = {},
                    endFrame = {},
                    codeBreak = {},
                    stateLoaded = {},
                    stateSaved = {},
                    inputPolled = {},
                    spriteZeroHit = {},
                    scriptEnded = {},
                },
            },
            executeCountType = {
                fields = {
                    cpuCycles = {},
                    ppuCycles = {},
                    cpuInstructions = {},
                },
            },
            memCallbackType = {
                fields = {
                    cpuRead = {},
                    cpuWrite = {},
                    cpuExec = {},
                    ppuRead = {},
                    ppuWrite = {},
                },
            },
            memType = {
                fields = {
                    cpu = {},
                    ppu = {},
                    palette = {},
                    oam = {},
                    secondaryOam = {},
                    prgRom = {},
                    chrRom = {},
                    chrRam = {},
                    workRam = {},
                    saveRam = {},
                    cpuDebug = {},
                    ppuDebug = {},
                },
            },
            counterMemType = {
                fields = {
                    nesRam = {},
                    prgRom = {},
                    workRam = {},
                    saveRam = {},
                },
            },
            counterOpType = {
                read = {},
                write = {},
                exec = {},
            },
        },
    },
}
