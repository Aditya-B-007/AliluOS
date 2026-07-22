use core::arch::asm;
use crate::vga::{Color, VGA};

/// Polls the keyboard I/O port for a scancode without blocking.
/// Returns Some(scancode) if a key is available, or None.
fn poll_scancode() -> Option<u8> {
    unsafe {
        let status: u8;
        asm!(
            "in al, dx",
            in("dx") 0x64u16,
            out("al") status,
            options(nomem, nostack)
        );
        if status & 1 != 0 {
            let scancode: u8;
            asm!(
                "in al, dx",
                in("dx") 0x60u16,
                out("al") scancode,
                options(nomem, nostack)
            );
            Some(scancode)
        } else {
            None
        }
    }
}

/// Blocks until a scancode is received.
fn read_scancode_blocking() -> u8 {
    loop {
        if let Some(code) = poll_scancode() {
            return code;
        }
    }
}

/// Translates a scancode to a char for algebraic chess input.
fn scancode_to_char(code: u8, shift: bool) -> Option<char> {
    match code {
        0x1E => Some(if shift { 'A' } else { 'a' }),
        0x30 => Some(if shift { 'B' } else { 'b' }),
        0x2E => Some(if shift { 'C' } else { 'c' }),
        0x20 => Some(if shift { 'D' } else { 'd' }),
        0x12 => Some(if shift { 'E' } else { 'e' }),
        0x21 => Some(if shift { 'F' } else { 'f' }),
        0x22 => Some(if shift { 'G' } else { 'g' }),
        0x23 => Some(if shift { 'H' } else { 'h' }),
        0x02 => Some('1'),
        0x03 => Some('2'),
        0x04 => Some('3'),
        0x05 => Some('4'),
        0x06 => Some('5'),
        0x07 => Some('6'),
        0x08 => Some('7'),
        0x09 => Some('8'),
        0x0A => Some('9'),
        0x0B => Some('0'),
        _ => None,
    }
}

// ----------------------------------------------------
// 1. DRAWING CANVAS TOOL
// ----------------------------------------------------

/// Launches the full-screen Drawing Canvas.
/// Background is pure black, drawings are white.
pub fn start_canvas() {
    let mut vga = VGA::new();
    vga.clear();
    vga.set_color(Color::White, Color::Black);

    vga.println("=== DRAWING CANVAS ===");
    vga.println("Use Arrow keys to move cursor.");
    vga.println("Space = Draw White Dot | Backspace/E = Erase | Escape = Exit");
    vga.println("Press any key to start...");
    read_scancode_blocking();
    vga.clear();

    let mut cursor_x = 40;
    let mut cursor_y = 12;
    
    // Grid memory to persist drawing (80x25 grid)
    let mut grid = [b' '; 80 * 25];

    loop {
        // Redraw persistent canvas grid
        for y in 0..25 {
            for x in 0..80 {
                // If cursor is on this position, show a blinking/highlighted character
                if x == cursor_x && y == cursor_y {
                    vga.put_char_at(y, x, '+');
                } else {
                    vga.put_char_at(y, x, grid[y * 80 + x] as char);
                }
            }
        }

        let scancode = read_scancode_blocking();
        match scancode {
            0x01 => break, // Escape: Exit
            0x48 => { // Up Arrow
                if cursor_y > 0 { cursor_y -= 1; }
            }
            0x50 => { // Down Arrow
                if cursor_y < 24 { cursor_y += 1; }
            }
            0x4B => { // Left Arrow
                if cursor_x > 0 { cursor_x -= 1; }
            }
            0x4D => { // Right Arrow
                if cursor_x < 79 { cursor_x += 1; }
            }
            0x39 => { // Space: Draw white block/dot
                grid[cursor_y * 80 + cursor_x] = b'*';
            }
            0x0E | 0x12 => { // Backspace or 'E': Erase
                grid[cursor_y * 80 + cursor_x] = b' ';
            }
            _ => {}
        }
    }
    vga.clear();
}

// ----------------------------------------------------
// 2. ATARI BREAKOUT GAME
// ----------------------------------------------------

/// Launches the full-screen Atari Breakout game.
pub fn start_atari() {
    let mut vga = VGA::new();
    vga.clear();
    vga.set_color(Color::White, Color::Black);

    vga.println("=== ATARI BREAKOUT ===");
    vga.println("Left/Right Arrow keys = Move Paddle");
    vga.println("Escape = Exit");
    vga.println("Press any key to start...");
    read_scancode_blocking();
    vga.clear();

    let mut paddle_x = 35; // Paddle center
    let paddle_width = 10;
    
    let mut ball_x = 40;
    let mut ball_y = 15;
    let mut ball_dx = 1;
    let mut ball_dy = -1;

    let mut score = 0;
    let mut lives = 3;

    // Bricks grid (5 rows, 10 columns of bricks)
    // Brick width is 6 chars, spaced.
    let mut bricks = [true; 50]; 

    loop {
        // Clear frame buffer
        vga.clear();

        // 1. Draw Bricks
        for row in 0..5 {
            for col in 0..10 {
                if bricks[row * 10 + col] {
                    vga.set_color(Color::LightGray, Color::Black);
                    let start_x = 10 + col * 6;
                    let start_y = 3 + row;
                    for offset in 0..5 {
                        vga.put_char_at(start_y, start_x + offset, '=');
                    }
                }
            }
        }

        vga.set_color(Color::White, Color::Black);

        // 2. Draw Score & Interface
        vga.put_char_at(1, 2, 'S');
        vga.put_char_at(1, 3, 'C');
        vga.put_char_at(1, 4, 'O');
        vga.put_char_at(1, 5, 'R');
        vga.put_char_at(1, 6, 'E');
        vga.put_char_at(1, 7, ':');
        print_num_at(1, 9, score, &vga);

        vga.put_char_at(1, 70, 'L');
        vga.put_char_at(1, 71, 'I');
        vga.put_char_at(1, 72, 'V');
        vga.put_char_at(1, 73, 'E');
        vga.put_char_at(1, 74, 'S');
        vga.put_char_at(1, 75, ':');
        vga.put_char_at(1, 77, (b'0' + lives as u8) as char);

        // 3. Draw Paddle
        for offset in 0..paddle_width {
            let px = paddle_x + offset;
            if px < 80 {
                vga.put_char_at(22, px, '=');
            }
        }

        // 4. Draw Ball
        vga.put_char_at(ball_y, ball_x, 'O');

        // Check controls (Non-blocking poll)
        if let Some(scancode) = poll_scancode() {
            match scancode {
                0x01 => break, // Escape: Exit
                0x4B => { // Left Arrow
                    if paddle_x > 2 { paddle_x -= 3; }
                }
                0x4D => { // Right Arrow
                    if paddle_x + paddle_width < 78 { paddle_x += 3; }
                }
                _ => {}
            }
        }

        // Delay physics ticks
        delay(4000000);

        // 5. Physics: Ball movement
        ball_x = (ball_x as i32 + ball_dx) as usize;
        ball_y = (ball_y as i32 + ball_dy) as usize;

        // Bounce walls
        if ball_x <= 1 {
            ball_x = 2;
            ball_dx = -ball_dx;
        } else if ball_x >= 78 {
            ball_x = 77;
            ball_dx = -ball_dx;
        }

        if ball_y <= 2 {
            ball_y = 3;
            ball_dy = -ball_dy;
        }

        // Brick collision
        if ball_y >= 3 && ball_y < 8 {
            let row = ball_y - 3;
            if ball_x >= 10 && ball_x < 70 {
                let col = (ball_x - 10) / 6;
                let idx = row * 10 + col;
                if idx < 50 && bricks[idx] {
                    bricks[idx] = false;
                    ball_dy = -ball_dy;
                    score += 10;
                }
            }
        }

        // Paddle collision
        if ball_y == 22 {
            if ball_x >= paddle_x && ball_x <= paddle_x + paddle_width {
                ball_dy = -ball_dy;
                // Add slight angle variance depending on where it hits paddle
                if ball_x < paddle_x + 3 {
                    ball_dx = -1;
                } else if ball_x > paddle_x + 7 {
                    ball_dx = 1;
                }
            }
        }

        // Missed ball
        if ball_y >= 24 {
            lives -= 1;
            if lives == 0 {
                vga.clear();
                vga.set_color(Color::LightRed, Color::Black);
                vga.println("\n=== GAME OVER ===");
                vga.write("Final Score: ");
                print_num_at(3, 14, score, &vga);
                vga.println("\nPress any key to return to shell...");
                read_scancode_blocking();
                break;
            }
            // Reset ball
            ball_x = paddle_x + paddle_width / 2;
            ball_y = 20;
            ball_dy = -1;
        }

        // Win check
        if score >= 500 {
            vga.clear();
            vga.set_color(Color::LightGreen, Color::Black);
            vga.println("\n=== YOU WIN! ===");
            vga.println("Excellent job! You destroyed all bricks.");
            vga.println("\nPress any key to return to shell...");
            read_scancode_blocking();
            break;
        }
    }
    vga.clear();
}

// Helper to print positive numbers on VGA
fn print_num_at(row: usize, col: usize, val: usize, vga: &VGA) {
    let mut temp = val;
    if temp == 0 {
        vga.put_char_at(row, col, '0');
        return;
    }
    let mut digits = [0u8; 10];
    let mut idx = 0;
    while temp > 0 {
        digits[idx] = (temp % 10) as u8;
        temp /= 10;
        idx += 1;
    }
    for offset in 0..idx {
        vga.put_char_at(row, col + offset, (b'0' + digits[idx - 1 - offset]) as char);
    }
}

// Simple busy-wait loop delay helper
fn delay(n: usize) {
    for _ in 0..n {
        unsafe { asm!("nop", options(nomem, nostack)); }
    }
}

// ----------------------------------------------------
// 3. CHESS GAME
// ----------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Piece {
    Pawn, Knight, Bishop, Rook, Queen, King, Empty
}

#[derive(Clone, Copy, PartialEq)]
enum ChessColor {
    White, Black
}

#[derive(Clone, Copy)]
struct ChessPiece {
    piece: Piece,
    color: ChessColor,
}

/// Launches the full-screen two-player Chess game.
pub fn start_chess() {
    let mut vga = VGA::new();
    vga.clear();
    vga.set_color(Color::White, Color::Black);

    vga.println("=== CHESS ===");
    vga.println("Two-Player mode.");
    vga.println("Move inputs are algebraic coordinates (e.g., 'e2e4' or 'b1c3').");
    vga.println("Type 'exit' to quit back to shell.");
    vga.println("Press any key to start...");
    read_scancode_blocking();

    // Default Chess Board setup
    let mut board = [[ChessPiece { piece: Piece::Empty, color: ChessColor::White }; 8]; 8];

    // Setup base pieces
    let back_row = [Piece::Rook, Piece::Knight, Piece::Bishop, Piece::Queen, Piece::King, Piece::Bishop, Piece::Knight, Piece::Rook];
    for col in 0..8 {
        board[0][col] = ChessPiece { piece: back_row[col], color: ChessColor::Black };
        board[1][col] = ChessPiece { piece: Piece::Pawn, color: ChessColor::Black };
        board[6][col] = ChessPiece { piece: Piece::Pawn, color: ChessColor::White };
        board[7][col] = ChessPiece { piece: back_row[col], color: ChessColor::White };
    }

    let mut turn = ChessColor::White;
    let mut input_buffer = alloc::string::String::new();

    loop {
        vga.clear();
        draw_board(&board, &vga);

        // Instructions and turn information
        vga.set_color(Color::LightCyan, Color::Black);
        vga.put_char_at(3, 45, 'T');
        vga.put_char_at(3, 46, 'U');
        vga.put_char_at(3, 47, 'R');
        vga.put_char_at(3, 48, 'N');
        vga.put_char_at(3, 49, ':');
        if turn == ChessColor::White {
            vga.set_color(Color::White, Color::Black);
            vga.put_char_at(3, 51, 'W');
            vga.put_char_at(3, 52, 'h');
            vga.put_char_at(3, 53, 'i');
            vga.put_char_at(3, 54, 't');
            vga.put_char_at(3, 55, 'e');
        } else {
            vga.set_color(Color::LightGray, Color::Black);
            vga.put_char_at(3, 51, 'B');
            vga.put_char_at(3, 52, 'l');
            vga.put_char_at(3, 53, 'a');
            vga.put_char_at(3, 54, 'c');
            vga.put_char_at(3, 55, 'k');
        }

        vga.set_color(Color::White, Color::Black);
        vga.put_char_at(20, 2, 'Y');
        vga.put_char_at(20, 3, 'o');
        vga.put_char_at(20, 4, 'u');
        vga.put_char_at(20, 5, 'r');
        vga.put_char_at(20, 6, ' ');
        vga.put_char_at(20, 7, 'M');
        vga.put_char_at(20, 8, 'o');
        vga.put_char_at(20, 9, 'v');
        vga.put_char_at(20, 10, 'e');
        vga.put_char_at(20, 11, ':');
        vga.put_char_at(20, 13, ' ');

        // Draw current input string on screen
        for (i, c) in input_buffer.chars().enumerate() {
            vga.put_char_at(20, 14 + i, c);
        }

        // Wait for key
        let scancode = read_scancode_blocking();
        match scancode {
            0x01 => break, // Escape: Exit
            0x1C => { // Enter: Commit input
                if input_buffer == "exit" {
                    break;
                }
                if input_buffer.len() == 4 {
                    let chars: Vec<char> = input_buffer.chars().collect();
                    let start_col = (chars[0] as u8 - b'a') as usize;
                    let start_row = (b'8' - chars[1] as u8) as usize;
                    let end_col = (chars[2] as u8 - b'a') as usize;
                    let end_row = (b'8' - chars[3] as u8) as usize;

                    if start_col < 8 && start_row < 8 && end_col < 8 && end_row < 8 {
                        let selected = board[start_row][start_col];
                        if selected.piece != Piece::Empty && selected.color == turn {
                            // Perform simple move (no complex verification for simplicity)
                            board[end_row][end_col] = selected;
                            board[start_row][start_col] = ChessPiece { piece: Piece::Empty, color: ChessColor::White };
                            // Switch turn
                            turn = if turn == ChessColor::White { ChessColor::Black } else { ChessColor::White };
                        }
                    }
                }
                input_buffer.clear();
            }
            0x0E => { // Backspace
                input_buffer.pop();
            }
            _ => {
                if let Some(c) = scancode_to_char(scancode, false) {
                    if input_buffer.len() < 10 {
                        input_buffer.push(c);
                    }
                }
            }
        }
    }
    vga.clear();
}

/// Renders the chess board onto the screen.
fn draw_board(board: &[[ChessPiece; 8]; 8], vga: &VGA) {
    vga.set_color(Color::White, Color::Black);
    
    // Draw columns letters
    vga.put_char_at(1, 8, 'A');
    vga.put_char_at(1, 13, 'B');
    vga.put_char_at(1, 18, 'C');
    vga.put_char_at(1, 23, 'D');
    vga.put_char_at(1, 28, 'E');
    vga.put_char_at(1, 33, 'F');
    vga.put_char_at(1, 38, 'G');
    vga.put_char_at(1, 43, 'H');

    // Draw board content rows
    for r in 0..8 {
        vga.set_color(Color::White, Color::Black);
        // Draw row rank number
        vga.put_char_at(3 + r * 2, 2, (b'8' - r as u8) as char);

        for c in 0..8 {
            // Draw alternating squares visually using colors
            let bg = if (r + c) % 2 == 0 { Color::DarkGray } else { Color::Black };
            
            let p = board[r][c];
            let fg = if p.color == ChessColor::White { Color::White } else { Color::LightRed };
            vga.set_color(fg, bg);

            // Piece characters
            let (c1, c2) = match p.piece {
                Piece::Pawn => ('P', ' '),
                Piece::Knight => ('N', ' '),
                Piece::Bishop => ('B', ' '),
                Piece::Rook => ('R', ' '),
                Piece::Queen => ('Q', ' '),
                Piece::King => ('K', ' '),
                Piece::Empty => ('-', '-'),
            };

            let start_x = 6 + c * 5;
            let start_y = 3 + r * 2;
            
            vga.put_char_at(start_y, start_x, ' ');
            vga.put_char_at(start_y, start_x + 1, c1);
            vga.put_char_at(start_y, start_x + 2, c2);
            vga.put_char_at(start_y, start_x + 3, ' ');
        }
    }
}
