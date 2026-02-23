mod systems;
mod ui;

#[path = "../chess_engine.rs"]
mod chess_engine;

fn main() {
    let mut game = chess_engine::new_game();
    let best = chess_engine::reply(&mut game);
    println!(
        "best move: {} -> {} (score {})",
        best.src, best.dst, best.score
    );
}
