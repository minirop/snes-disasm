use byteorder::*;
use clap::{Parser, ValueEnum};
use clap_num::maybe_hex;
use std::fs::File;
use std::io::{self, Seek, SeekFrom};

#[allow(unused)]
#[derive(Debug, Clone, ValueEnum)]
enum State {
    Rep,
    Sep,
}

#[derive(Debug, Parser)]
struct Args {
    filename: String,

    #[arg(short, long, value_parser=maybe_hex::<u32>)]
    start: u32,

    #[arg(short, long, value_parser=maybe_hex::<u32>)]
    end: u32,

    #[arg(short = 'r', long)]
    state: Option<State>,
}

#[allow(unused)]
enum RepSep {
    Rep(u8),
    Sep(u8),
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut file = File::open(&args.filename)?;

    let offset = args.start - 0x808000;
    file.seek(SeekFrom::Start(offset as u64))?;
    let end = (offset + args.end - args.start + 1) as u64;

    let mut rep_or_sep = if let Some(state) = args.state {
        match state {
            State::Rep => RepSep::Rep(0),
            State::Sep => RepSep::Sep(0),
        }
    } else {
        RepSep::Rep(0)
    };

    let mut labels = vec![format!("L{:06X}", args.start)];
    let mut assembly = vec![];
    let mut current_address = args.start;
    while file.seek(SeekFrom::Current(0))? < end {
        let opcode = file.read_u8()?;
        let addr = current_address;

        current_address += 1;
        let asm = match opcode {
            0x05 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("ora ${value:02X}")
            }
            0x06 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("asl ${value:02X}")
            }
            0x08 => format!("php"),
            0x09 => match rep_or_sep {
                RepSep::Rep(_) => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!("ora #${value:04X}")
                }
                RepSep::Sep(_) => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!("ora #${value:02X}")
                }
            },
            0x0A => format!("asl"),
            0x10 => {
                current_address += 1;
                let value = file.read_i8()?;
                let new_address = current_address as i32 + value as i32;
                labels.push(format!("L{:06X}", new_address as u32));
                format!("bpl L{:06X}", new_address as u32)
            }
            0x18 => format!("clc"),
            0x19 => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("ora.w ${value:04X}, Y")
            }
            0x1A => format!("inc A"),
            0x20 => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("jsr ${value:04X}")
            }
            0x22 => {
                current_address += 3;
                let value = file.read_u24::<LittleEndian>()?;
                labels.push(format!("L{value:06X}"));
                format!("jsl L{value:06X}")
            }
            0x28 => format!("plp"),
            0x29 => match rep_or_sep {
                RepSep::Rep(_) => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!("and #${value:02X}")
                }
                RepSep::Sep(_) => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!("and.w #${value:04X}")
                }
            },
            0x30 => {
                current_address += 1;
                let value = file.read_i8()?;
                let new_address = current_address as i32 + value as i32;
                labels.push(format!("L{:06X}", new_address as u32));
                format!("bmi L{:06X}", new_address as u32)
            }
            0x39 => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("and.w ${value:04X}, Y")
            }
            0x3A => format!("dec A"),
            0x48 => format!("pha"),
            0x49 => match rep_or_sep {
                RepSep::Rep(_) => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!("eor.w #${value:02X}")
                }
                RepSep::Sep(_) => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!("eor #${value:02X}")
                }
            },
            0x4A => format!("lsr"),
            0x4B => format!("phk"),
            0x4C => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("jmp ${value:04X}")
            }
            0x5A => format!("phy"),
            0x60 => format!("rts"),
            0x64 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("stz ${value:02X}")
            }
            0x65 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("adc ${value:02X}")
            }
            0x68 => format!("pla"),
            0x69 => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("adc.w #${value:04X}")
            }
            0x6B => format!("rtl"),
            0x70 => {
                current_address += 1;
                let value = file.read_i8()?;
                let new_address = current_address as i32 + value as i32;
                labels.push(format!("L{:06X}", new_address as u32));
                format!("bvs L{:06X}", new_address as u32)
            }
            0x7A => format!("ply"),
            0x7F => {
                current_address += 3;
                let value = file.read_u24::<LittleEndian>()?;
                format!("adc.l ${value:06X}, X")
            }
            0x80 => {
                current_address += 1;
                let value = file.read_i8()?;
                let new_address = current_address as i32 + value as i32;
                labels.push(format!("L{:06X}", new_address as u32));
                format!("bra L{:06X}", new_address as u32)
            }
            0x82 => {
                current_address += 2;
                let value = file.read_i16::<LittleEndian>()?;
                let new_address = current_address as i32 + value as i32;
                labels.push(format!("L{:06X}", new_address as u32));
                format!("brl L{:06X}", new_address as u32)
            }
            0x85 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("sta ${value:02X}")
            }
            0x89 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("bit #${value:02X}")
            }
            0x8A => format!("txa"),
            0x8B => format!("phb"),
            0x8D => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("sta.w ${value:04X}")
            }
            0x8F => {
                current_address += 3;
                let value = file.read_u24::<LittleEndian>()?;
                format!("sta.l ${value:06X}")
            }
            0x90 => {
                current_address += 1;
                let value = file.read_i8()?;
                let new_address = current_address as i32 + value as i32;
                labels.push(format!("L{:06X}", new_address as u32));
                format!("bcc L{:06X}", new_address as u32)
            }
            0x98 => format!("tya"),
            0x9D => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("sta.w ${value:04X}, X")
            }
            0x9F => {
                current_address += 3;
                let value = file.read_u24::<LittleEndian>()?;
                format!("sta.l ${value:06X}, X")
            }
            0xA0 => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("ldy.w #${value:04X}")
            }
            0xA2 => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("ldx.w #${value:04X}")
            }
            0xA4 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("ldy ${value:02X}")
            }
            0xA5 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("lda ${value:02X}")
            }
            0xA6 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("ldx ${value:02X}")
            }
            0xA8 => format!("tay"),
            0xA9 => match rep_or_sep {
                RepSep::Rep(_) => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!("lda.w #${value:04X}")
                }
                RepSep::Sep(_) => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!("lda #${value:02X}")
                }
            },
            0xAA => format!("tax"),
            0xAB => format!("plb"),
            0xAD => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("lda.w ${value:04X}")
            }
            0xAF => {
                current_address += 3;
                let value = file.read_u24::<LittleEndian>()?;
                format!("lda.l ${value:06X}")
            }
            0xB0 => {
                current_address += 1;
                let value = file.read_i8()?;
                let new_address = current_address as i32 + value as i32;
                labels.push(format!("L{:06X}", new_address as u32));
                format!("bcs L{:06X}", new_address as u32)
            }
            0xB7 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("lda [${value:02X}], Y")
            }
            0xBB => format!("tyx"),
            0xBD => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("lda.w ${value:04X}, X")
            }
            0xBF => {
                current_address += 3;
                let value = file.read_u24::<LittleEndian>()?;
                format!("lda.l ${value:06X}, X")
            }
            0xC0 => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("cpy.w #${value:04X}")
            }
            0xC2 => {
                current_address += 1;
                let value = file.read_u8()?;
                rep_or_sep = RepSep::Rep(value);
                format!("rep #${value:02X}")
            }
            0xC6 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("dec ${value:02X}")
            }
            0xC8 => format!("iny"),
            0xC9 => match rep_or_sep {
                RepSep::Rep(_) => {
                    current_address += 2;
                    let value = file.read_u16::<LittleEndian>()?;
                    format!("cmp.w #${value:04X}")
                }
                RepSep::Sep(_) => {
                    current_address += 1;
                    let value = file.read_u8()?;
                    format!("cmp #${value:02X}")
                }
            },
            0xD0 => {
                current_address += 1;
                let value = file.read_i8()?;
                let new_address = current_address as i32 + value as i32;
                labels.push(format!("L{:06X}", new_address as u32));
                format!("bne L{:06X}", new_address as u32)
            }
            0xDA => format!("phx"),
            0xDC => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("jml [${value:04X}]")
            }
            0xE0 => {
                current_address += 2;
                let value = file.read_u16::<LittleEndian>()?;
                format!("cpx.w #${value:04X}")
            }
            0xE2 => {
                current_address += 1;
                let value = file.read_u8()?;
                rep_or_sep = RepSep::Sep(value);
                format!("sep #${value:02X}")
            }
            0xE6 => {
                current_address += 1;
                let value = file.read_u8()?;
                format!("inc ${value:02X}")
            }
            0xEB => format!("xba"),
            0xF0 => {
                current_address += 1;
                let value = file.read_u8()?;
                labels.push(format!("L{:06X}", current_address + value as u32));
                format!("beq L{:06X}", current_address + value as u32)
            }
            0xFA => format!("plx"),
            _ => {
                println!("{assembly:?}");
                panic!("unknown opcode: {opcode:#X}");
            }
        };

        assembly.push((format!("L{addr:06X}"), asm));
    }

    for (label, asm) in assembly {
        if labels.contains(&label) {
            println!("{label}:");
        }
        println!("\t{asm}");
    }

    Ok(())
}
