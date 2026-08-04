//! # AliluOS Built-in Text Games & Canvas Application (`ring2/game.rs`)

#![allow(dead_code)]

use crate::vga::{Color, WRITER, VGA};
use crate::keyboard::Key;

pub enum GameMode {
    None,
    Atari,
    Chess,
}

pub struct GameEngine {
    pub mode: GameMode,
    pub score: u32,
    pub player_x: usize,
    pub ball_x: usize,
    pub ball_y: usize,
    pub ball_dx: isize,
    pub ball_dy: isize,
}

impl GameEngine {
    pub const fn new() -> Self {
        Self {
            mode: GameMode::None,
            score: 0,
            player_x: 35,
            ball_x: 40,
            ball_y: 12,
            ball_dx: 1,
            ball_dy: -1,
        }
    }

    pub fn start_atari(&mut self) {
        self.mode = GameMode::Atari;
        self.score = 0;
        self.player_x = 35;
        self.ball_x = 40;
        self.ball_y = 12;
        self.ball_dx = 1;
        self.ball_dy = -1;
        self.render_atari_screen();
    }

    pub fn start_chess(&mut self) {
        self.mode = GameMode::Chess;
        self.render_chess_board();
    }

    pub fn handle_key(&mut self, key: Key) {
        match self.mode {
            GameMode::Atari => match key {
                Key::Left => {
                    if self.player_x > 2 {
                        self.player_x -= 2;
                        self.render_atari_screen();
                    }
                }
                Key::Right => {
                    if self.player_x < 68 {
                        self.player_x += 2;
                        self.render_atari_screen();
                    }
                }
                _ => {}
            },
            GameMode::Chess => {}
            GameMode::None => {}
        }
    }

    fn render_atari_screen(&self) {
        let mut vga = WRITER.lock();
        vga.clear();
        vga.set_color(Color::LightCyan, Color::Black);
        vga.println("=== AliluOS Atari Breakout Text Game ===");
        vga.set_color(Color::White, Color::Black);
        vga.println("Use Left/Right arrow keys to move paddle. Press Escape to exit.\n");

        for y in 3..6 {
            for x in 10..70 {
                vga.put_char_at(y, x, '#');
            }
        }

        vga.put_char_at(self.ball_y, self.ball_x, 'O');

        for x in 0..10 {
            vga.put_char_at(22, self.player_x + x, '=');
        }
    }

    fn render_chess_board(&self) {
        let mut vga = WRITER.lock();
        vga.clear();
        vga.set_color(Color::Yellow, Color::Black);
        vga.println("=== AliluOS Interactive Text Chess ===");
        vga.set_color(Color::White, Color::Black);
        vga.println("   a b c d e f g h");
        vga.println("8  r n b q k b n r  8");
        vga.println("7  p p p p p p p p  7");
        vga.println("6  . . . . . . . .  6");
        vga.println("5  . . . . . . . .  5");
        vga.println("4  . . . . . . . .  4");
        vga.println("3  . . . . . . . .  3");
        vga.println("2  P P P P P P P P  2");
        vga.println("1  R N B Q K B N R  1");
        vga.println("   a b c d e f g h");
        vga.println("\nPress Escape to exit chess mode.");
    }
}

pub static GAME: crate::vga::Locked<GameEngine> = crate::vga::Locked::new(GameEngine::new());
