// Example FSM in Rust admitted by v0.1 compiler

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Standing,
    Running,
    Jumping,
    Falling,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlayerAction {
    Move,
    Jump,
    Land,
    Stop,
}

pub fn update_player(state: PlayerState, action: PlayerAction) -> PlayerState {
    match (state, action) {
        (PlayerState::Standing, PlayerAction::Move) => PlayerState::Running,
        (PlayerState::Running, PlayerAction::Stop) => PlayerState::Standing,
        (PlayerState::Standing, PlayerAction::Jump) => PlayerState::Jumping,
        (PlayerState::Running, PlayerAction::Jump) => PlayerState::Jumping,
        (PlayerState::Jumping, PlayerAction::Land) => PlayerState::Standing,
        (PlayerState::Jumping, PlayerAction::Stop) => PlayerState::Falling,
        (PlayerState::Falling, PlayerAction::Land) => PlayerState::Standing,
        _ => state,
    }
}

pub fn calculate_score(base: i32, multiplier: i32, bonus: i32) -> i32 {
    let subtotal = base.wrapping_mul(multiplier);
    subtotal.wrapping_add(bonus)
}
