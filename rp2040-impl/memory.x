MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100 - 96K
    INSTRS : ORIGIN = 0x101E8000, LENGTH = 96K
    RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}

EXTERN(BOOT2_FIRMWARE)

SECTIONS {
    /* ### Boot loader */
    .boot2 ORIGIN(BOOT2) :
    {
        KEEP(*(.boot2));
    } > BOOT2
} INSERT BEFORE .text;

SECTIONS {
    .instrs ORIGIN(INSTRS) :
    {
        __instrs_start = .;
        LONG(0xFFFFFFFF);
        FILL(0xFFFFFFFF);
        . = ORIGIN(INSTRS) + LENGTH(INSTRS);
        __instrs_end = .;
    } > INSTRS
} INSERT BEFORE .text;

ASSERT(SIZEOF(.instrs) == LENGTH(INSTRS), "ERROR: .instrs must fill the whole INSTRS region");
ASSERT(__instrs_start % 4096 == 0, "ERROR: INSTRS must start on a 4 KiB flash sector");
