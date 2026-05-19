use byteorder::*;
use clap::Parser;
use clap_num::maybe_hex;
use std::fs::File;
use std::io::{self, Seek, SeekFrom};

#[derive(Debug, Parser)]
struct Args {
    filename: String,

    #[arg(short, long, value_parser=maybe_hex::<u32>)]
    start: u32,

    #[arg(short, long, value_parser=maybe_hex::<u32>)]
    end: u32,

    // if set A starts as 8-bit
    #[arg(short, long)]
    accu: bool,

    // if set X and Y starts as 8-bit
    #[arg(short, long)]
    index: bool,

    // prints lowercase opcodes
    #[arg(short, long)]
    lower: bool,

    // stops when hitting a BRK opcode
    #[arg(short, long)]
    brk: bool,

    // stops when hitting specific opcodes (WAI, COP, and BRK)
    #[arg(short = 'k', long)]
    stop: bool,
}

const END_OF_BLOCK: [&str; 5] = ["RTS", "RTL", "JMP", "BRA", "BRL"];
const MAYBE_INVALID_OPCODES: [&str; 3] = ["COP", "WAI", "BRK"];

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut file = File::open(&args.filename)?;

    let bank = (args.start >> 16) - 0x80;
    let offset = ((args.start & 0xFFFF) - 0x8000) + bank * 0x8000;
    file.seek(SeekFrom::Start(offset as u64))?;
    let end = (offset + args.end - args.start + 1) as u64;

    let mut accu_is_16bits = !args.accu;
    let mut indexes_are_16bits = !args.index;
    let lowercase_opcodes = args.lower;
    let stop_on_brk = args.brk;
    let stop_on_sus_opcodes = args.stop;

    let mut labels = vec![format!("L{:06X}", args.start)];
    let mut assembly = vec![];
    let mut current_address = args.start;
    while file.seek(SeekFrom::Current(0))? < end {
        let opcode = file.read_u8()?;
        let addr = current_address;
        print!("{current_address:06X}: ");

        current_address += 1;
        if let Some(Some(opcode)) = OPCODES.get(opcode as usize) {
            let asm = opcode.name;
            let follow = match &opcode.addressing {
                Addressing::Absolute => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!(" ${value:04X}")
                }
                Addressing::AbsoluteIndirect => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!(" (${value:04X})")
                }
                Addressing::AbsoluteIndirectLong => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!(" [${value:04X}]")
                }
                Addressing::AbsoluteLong => {
                    current_address += 3;
                    let value = file.read_u24::<LittleEndian>()?;
                    format!(" ${value:06X}")
                }
                Addressing::AbsoluteX => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!(" ${value:04X},X")
                }
                Addressing::AbsoluteLongX => {
                    current_address += 3;
                    let value = file.read_u24::<LittleEndian>()?;
                    format!(" ${value:06X},X")
                }
                Addressing::AbsoluteY => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!(" ${value:04X},Y")
                }
                Addressing::AbsoluteXIndirect => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!(" (${value:04X},X)")
                }
                Addressing::Accumulator => format!(" A"),
                Addressing::Immediate(register) => {
                    if (*register == Register::Accumulator && accu_is_16bits)
                        || (*register == Register::Indexes && indexes_are_16bits)
                    {
                        current_address += 2;
                        let value = file.read_u16::<LittleEndian>()?;
                        format!(" #${value:04X}")
                    } else {
                        current_address += 1;
                        let value = file.read_u8()?;
                        format!(" #${value:02X}")
                    }
                }
                Addressing::Immediate8 => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    if opcode.name == "SEP" {
                        if (value & 0x20) > 0 {
                            accu_is_16bits = false;
                        }
                        if (value & 0x10) > 0 {
                            indexes_are_16bits = false;
                        }
                    } else if opcode.name == "REP" {
                        if (value & 0x20) > 0 {
                            accu_is_16bits = true;
                        }
                        if (value & 0x10) > 0 {
                            indexes_are_16bits = true;
                        }
                    }
                    format!(" #${value:02X}")
                }
                Addressing::Implied => {
                    format!("")
                }
                Addressing::Indirect => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" (${value:02X})")
                }
                Addressing::IndirectLongY => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" [${value:02X}],Y")
                }
                Addressing::IndirectY => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" (${value:02X}),Y")
                }
                Addressing::Relative => {
                    current_address += 1;
                    let value = file.read_i8()?;
                    let target = current_address.strict_add_signed(value as i32);
                    labels.push(format!("L{target:06X}"));
                    format!(" L{target:06X}")
                }
                Addressing::RelativeLong => {
                    current_address += 2;
                    let value = file.read_i16::<LittleEndian>()?;
                    let target = current_address.strict_add_signed(value as i32);
                    labels.push(format!("L{target:06X}"));
                    format!(" L{target:06X}")
                }
                Addressing::StackRelative => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" ${value:02X},S")
                }
                Addressing::XIndirect => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" (${value:02X},X)")
                }
                Addressing::ZeroPage => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" ${value:02X}")
                }
                Addressing::ZeroPageLong => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" [${value:02X}]")
                }
                Addressing::ZeroPageX => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" ${value:02X},X")
                }
                Addressing::ZeroPageY => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!(" ${value:02X},Y")
                }
            };

            if stop_on_brk && asm == "BRK" {
                println!("BRK hit!");
                return Ok(());
            }

            if stop_on_sus_opcodes && MAYBE_INVALID_OPCODES.contains(&asm) {
                println!("{asm} hit!");
                return Ok(());
            }

            let is_end_of_block = END_OF_BLOCK.contains(&asm);

            let asm = if lowercase_opcodes {
                asm.to_lowercase()
            } else {
                asm.to_string()
            };

            let output = format!("{asm}{follow}");
            println!("{asm}{follow}");
            assembly.push((format!("L{addr:06X}"), output));

            if is_end_of_block {
                labels.push(format!("L{current_address:06X}"));
                assembly.push(("".into(), "".into()));
            }
        } else {
            panic!("{opcode:#X} unhandled at {:06X}", current_address - 1);
        }
    }

    for (label, asm) in assembly {
        if labels.contains(&label) {
            println!("{label}:");
        }
        println!("\t{asm}");
    }

    Ok(())
}

#[derive(Debug)]
enum Addressing {
    Absolute,
    AbsoluteIndirect,
    AbsoluteIndirectLong,
    AbsoluteLong,
    AbsoluteLongX,
    AbsoluteX,
    AbsoluteY,
    AbsoluteXIndirect,
    Accumulator,
    Immediate(Register),
    Immediate8,
    Implied,
    Indirect,
    IndirectLongY,
    IndirectY,
    Relative,
    RelativeLong,
    StackRelative,
    XIndirect,
    ZeroPage,
    ZeroPageLong,
    ZeroPageX,
    ZeroPageY,
}

#[derive(Debug, PartialEq)]
enum Register {
    Accumulator,
    Indexes,
}

#[derive(Debug)]
struct Opcode {
    name: &'static str,
    addressing: Addressing,
}

const OPCODES: [Option<Opcode>; 256] = [
    // 0x00
    Some(Opcode {
        name: "BRK",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::XIndirect,
    }),
    Some(Opcode {
        name: "COP",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::StackRelative,
    }),
    Some(Opcode {
        name: "TSB",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "ASL",
        addressing: Addressing::ZeroPage,
    }),
    None,
    Some(Opcode {
        name: "PHP",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::Immediate(Register::Accumulator),
    }),
    Some(Opcode {
        name: "ASL",
        addressing: Addressing::Accumulator,
    }),
    Some(Opcode {
        name: "PHD",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "TSB",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "ASL",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::AbsoluteLong,
    }),
    // 0x10
    Some(Opcode {
        name: "BPL",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::IndirectY,
    }),
    None,
    None,
    Some(Opcode {
        name: "TRB",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "ASL",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::IndirectLongY,
    }),
    Some(Opcode {
        name: "CLC",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "INC",
        addressing: Addressing::Accumulator,
    }),
    None,
    Some(Opcode {
        name: "TRB",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "ORA",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "ASL",
        addressing: Addressing::AbsoluteX,
    }),
    None,
    // 0x20
    Some(Opcode {
        name: "JSR",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "AND",
        addressing: Addressing::XIndirect,
    }),
    Some(Opcode {
        name: "JSL",
        addressing: Addressing::AbsoluteLong,
    }),
    None,
    Some(Opcode {
        name: "BIT",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "AND",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "ROL",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "AND",
        addressing: Addressing::ZeroPageLong,
    }),
    Some(Opcode {
        name: "PLP",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "AND",
        addressing: Addressing::Immediate(Register::Accumulator),
    }),
    Some(Opcode {
        name: "ROL",
        addressing: Addressing::Accumulator,
    }),
    None,
    Some(Opcode {
        name: "BIT",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "AND",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "ROL",
        addressing: Addressing::Absolute,
    }),
    None,
    // 0x30
    Some(Opcode {
        name: "BMI",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "AND",
        addressing: Addressing::IndirectY,
    }),
    None,
    None,
    None,
    Some(Opcode {
        name: "AND",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "ROL",
        addressing: Addressing::ZeroPageX,
    }),
    None,
    Some(Opcode {
        name: "SEC",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "AND",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "DEC",
        addressing: Addressing::Accumulator,
    }),
    None,
    None,
    Some(Opcode {
        name: "AND",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "ROL",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "AND",
        addressing: Addressing::AbsoluteLongX,
    }),
    // 0x40
    Some(Opcode {
        name: "RTI",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::XIndirect,
    }),
    None,
    None,
    None,
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "LSR",
        addressing: Addressing::ZeroPage,
    }),
    None,
    Some(Opcode {
        name: "PHA",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::Immediate(Register::Accumulator),
    }),
    Some(Opcode {
        name: "LSR",
        addressing: Addressing::Accumulator,
    }),
    None,
    Some(Opcode {
        name: "JMP",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "LSR",
        addressing: Addressing::Absolute,
    }),
    None,
    // 0x50
    Some(Opcode {
        name: "BVC",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::IndirectY,
    }),
    None,
    None,
    None,
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "LSR",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::IndirectLongY,
    }),
    Some(Opcode {
        name: "CLI",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "PHY",
        addressing: Addressing::Implied,
    }),
    None,
    Some(Opcode {
        name: "JML",
        addressing: Addressing::AbsoluteLong,
    }),
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "LSR",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "EOR",
        addressing: Addressing::AbsoluteLongX,
    }),
    // 0x60
    Some(Opcode {
        name: "RTS",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::XIndirect,
    }),
    None,
    None,
    Some(Opcode {
        name: "STZ",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "ROR",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::ZeroPageLong,
    }),
    Some(Opcode {
        name: "PLA",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::Immediate(Register::Accumulator),
    }),
    Some(Opcode {
        name: "ROR",
        addressing: Addressing::Accumulator,
    }),
    Some(Opcode {
        name: "RTL",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "JMP",
        addressing: Addressing::AbsoluteIndirect,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "ROR",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::AbsoluteLong,
    }),
    // 0x70
    Some(Opcode {
        name: "BVS",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::IndirectY,
    }),
    None,
    None,
    None,
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "ROR",
        addressing: Addressing::ZeroPageX,
    }),
    None,
    Some(Opcode {
        name: "SEI",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "PLY",
        addressing: Addressing::Implied,
    }),
    None,
    Some(Opcode {
        name: "JMP",
        addressing: Addressing::AbsoluteXIndirect,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "ROR",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "ADC",
        addressing: Addressing::AbsoluteLongX,
    }),
    // 0x80
    Some(Opcode {
        name: "BRA",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::XIndirect,
    }),
    Some(Opcode {
        name: "BRL",
        addressing: Addressing::RelativeLong,
    }),
    None,
    Some(Opcode {
        name: "STY",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "STX",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::ZeroPageLong,
    }),
    Some(Opcode {
        name: "DEY",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "BIT",
        addressing: Addressing::Immediate(Register::Accumulator),
    }),
    Some(Opcode {
        name: "TXA",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "PHB",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "STY",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "STX",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::AbsoluteLong,
    }),
    // 0x90
    Some(Opcode {
        name: "BCC",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::IndirectY,
    }),
    None,
    None,
    Some(Opcode {
        name: "STY",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "STX",
        addressing: Addressing::ZeroPageY,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::IndirectLongY,
    }),
    Some(Opcode {
        name: "TYA",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "TXS",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "TXY",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "STZ",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "STZ",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "STA",
        addressing: Addressing::AbsoluteLongX,
    }),
    // 0xA0
    Some(Opcode {
        name: "LDY",
        addressing: Addressing::Immediate(Register::Indexes),
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::XIndirect,
    }),
    Some(Opcode {
        name: "LDX",
        addressing: Addressing::Immediate(Register::Indexes),
    }),
    None,
    Some(Opcode {
        name: "LDY",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "LDX",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::ZeroPageLong,
    }),
    Some(Opcode {
        name: "TAY",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::Immediate(Register::Accumulator),
    }),
    Some(Opcode {
        name: "TAX",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "PLB",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "LDY",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "LDX",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::AbsoluteLong,
    }),
    // 0xB0
    Some(Opcode {
        name: "BCS",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::IndirectY,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::Indirect,
    }),
    None,
    Some(Opcode {
        name: "LDY",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "LDX",
        addressing: Addressing::ZeroPageY,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::IndirectLongY,
    }),
    Some(Opcode {
        name: "CLV",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "TSX",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "TYX",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "LDY",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "LDX",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "LDA",
        addressing: Addressing::AbsoluteLongX,
    }),
    // 0xC0
    Some(Opcode {
        name: "CPY",
        addressing: Addressing::Immediate(Register::Indexes),
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::XIndirect,
    }),
    Some(Opcode {
        name: "REP",
        addressing: Addressing::Immediate8,
    }),
    None,
    Some(Opcode {
        name: "CPY",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "DEC",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::ZeroPageLong,
    }),
    Some(Opcode {
        name: "INY",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::Immediate(Register::Accumulator),
    }),
    Some(Opcode {
        name: "DEX",
        addressing: Addressing::Implied,
    }),
    None,
    Some(Opcode {
        name: "CPY",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "DEC",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::AbsoluteLong,
    }),
    // 0xD0
    Some(Opcode {
        name: "BNE",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::IndirectY,
    }),
    None,
    None,
    None,
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "DEC",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::IndirectLongY,
    }),
    Some(Opcode {
        name: "CLD",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "PHX",
        addressing: Addressing::Implied,
    }),
    None,
    Some(Opcode {
        name: "JML",
        addressing: Addressing::AbsoluteIndirectLong,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "DEC",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "CMP",
        addressing: Addressing::AbsoluteLongX,
    }),
    // 0xE0
    Some(Opcode {
        name: "CPX",
        addressing: Addressing::Immediate(Register::Indexes),
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::XIndirect,
    }),
    Some(Opcode {
        name: "SEP",
        addressing: Addressing::Immediate8,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::StackRelative,
    }),
    Some(Opcode {
        name: "CPX",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::ZeroPage,
    }),
    Some(Opcode {
        name: "INC",
        addressing: Addressing::ZeroPage,
    }),
    None,
    Some(Opcode {
        name: "INX",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::Immediate(Register::Accumulator),
    }),
    Some(Opcode {
        name: "NOP",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "XBA",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "CPX",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "INC",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::AbsoluteLong,
    }),
    // 0xF0
    Some(Opcode {
        name: "BEQ",
        addressing: Addressing::Relative,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::IndirectY,
    }),
    None,
    None,
    Some(Opcode {
        name: "PEA",
        addressing: Addressing::Absolute,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "INC",
        addressing: Addressing::ZeroPageX,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::IndirectLongY,
    }),
    Some(Opcode {
        name: "SED",
        addressing: Addressing::Implied,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::AbsoluteY,
    }),
    Some(Opcode {
        name: "PLX",
        addressing: Addressing::Implied,
    }),
    None,
    Some(Opcode {
        name: "JSR",
        addressing: Addressing::AbsoluteXIndirect,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "INC",
        addressing: Addressing::AbsoluteX,
    }),
    Some(Opcode {
        name: "SBC",
        addressing: Addressing::AbsoluteLongX,
    }),
];
