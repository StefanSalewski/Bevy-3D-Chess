// "Chess Scene Pieces / Blender" (https://skfb.ly/67OF8) by moyicat is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).
// "Realistic 3D Chess Pieces (Blender)" (https://skfb.ly/oXTUP) by ronildo.facanha is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).
// "Chess" (https://skfb.ly/6uVLu) by xnicrox is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).
// "Wooden Chess Board" (https://skfb.ly/oXqwI) by Bhargav Limje is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).

// Plain Bevy frontend for the tiny Salewski chess engine
// v 0.2 — 15-AUG-2025
// (C) 2015 - 2032 Dr. Stefan Salewski
// All rights reserved.

// cargo run --features=default_font
// cargo run --features=salewskiChessDebug

// Bevy version 0.16.1

use bevy::color::palettes::tailwind::*;
use bevy::{
    color::palettes::css::GOLD,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

mod engine;

// === Constants ===
const DEFAULT_TIME_PER_MOVE: f32 = 1.5;
const ACCELERATION: f32 = 8.0; // pseudo-accel for arc animation
const MAX_SPEED: f32 = 6.0; // max piece animation speed

// highlight "kinds"
const SQUARE_HOVER_FACTOR: i32 = 1;
const PIECE_HOVER_FACTOR: i32 = 2;

// tag meanings in game_data.tagged
const TAG_LEGAL: i8 = 1;
const TAG_LAST: i8 = 2;

// Mapping from bool -> player label
const PLAYER_LABEL: [&str; 2] = ["Human", "Computer"];

// === Resources / Components ===
#[derive(Resource)]
struct NextMoveTask(Option<Task<engine::Move>>);

#[derive(Component, Reflect, Clone, Copy)]
struct PositionData {
    location: Vec3,
}

#[derive(Resource, Default, Component)]
struct Figure {
    location: Vec3,
    speed: f32,
}

#[derive(Resource)]
struct Txt {
    ui_text: String,
    turn: String,
    time: String,
    nxt: String,
}

impl Default for Txt {
    fn default() -> Self {
        Self {
            ui_text: "Left-click on a piece, then on a destination to move. Use the middle mouse\nbutton to rotate the board, right-click to pan, and scroll to zoom.".to_string(),
            turn: "Human player vs. Computer\n  use keypad 1 or 2 to change".to_string(),
            time: format!("{} secs per move\n  use keypad + or - to modify", DEFAULT_TIME_PER_MOVE),
            nxt: "White starts the game. Press\nkeypad 0 to start a new game.".to_string(),
        }
    }
}

impl Txt {
    fn refresh(&mut self, secs_per_move: f32, sides: &EnginePlays, game: &engine::Game) {
        self.turn = format!(
            "{} (1) vs {} (2)",
            PLAYER_LABEL[sides.t[0] as usize], PLAYER_LABEL[sides.t[1] as usize]
        );
        self.time = format!("Secs per move: {:.1}", secs_per_move);
        let next = game.move_counter as usize % 2;
        self.nxt = format!("Next move: {}", ["Black", "White"][next]);
    }
}

#[derive(Resource)]
struct EnginePlays {
    t: [bool; 2],
}

impl Default for EnginePlays {
    fn default() -> Self {
        Self { t: [false, true] }
    }
}

#[derive(Resource)]
struct SecsPerMove {
    time: f32,
}

impl Default for SecsPerMove {
    fn default() -> Self {
        Self {
            time: DEFAULT_TIME_PER_MOVE,
        }
    }
}

#[derive(Resource, PartialEq)]
enum State {
    Playing,
    Waiting,
    GameTerminated,
}

#[allow(dead_code)]
#[derive(Resource, Component)]
struct GameData {
    game: Arc<Mutex<engine::Game>>,
    tagged: engine::Board, // used for highlight of legal moves / last move
}

impl Default for GameData {
    fn default() -> Self {
        Self {
            game: Arc::new(Mutex::new(engine::new_game())),
            tagged: [0; 64],
        }
    }
}

#[derive(Resource, Default)]
struct SelectionState {
    first_selection: Option<(Entity, PositionData)>,
}

// Track last applied tags so we update materials only when needed
#[derive(Resource, Clone)]
struct TaggedCache(engine::Board);
impl Default for TaggedCache {
    fn default() -> Self {
        Self([0; 64])
    }
}

// --- Highlight support (swap-to-highlight with cache) ---

// Cache key: combine original material handle + variant so we can have multiple
// highlight looks per base material without clashes.
#[derive(Clone)]
struct HighlightKey {
    handle: Handle<StandardMaterial>,
    variant: i32,
}
impl PartialEq for HighlightKey {
    fn eq(&self, other: &Self) -> bool {
        self.variant == other.variant && self.handle == other.handle
    }
}
impl Eq for HighlightKey {}
impl Hash for HighlightKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.handle.hash(state);
        self.variant.hash(state);
    }
}

#[derive(Resource, Default)]
struct HighlightCache(HashMap<HighlightKey, Handle<StandardMaterial>>);

#[derive(Component, Clone)]
struct OriginalMaterial(Handle<StandardMaterial>);

// keep original square index for mapping
#[derive(Component, Copy, Clone)]
struct Square {
    idx: usize, // 0..63
}

fn unused_lighten_towards_white(color: Color, amount: f32) -> Color {
    let c = color.to_linear();
    let w = Color::srgb(1.0, 1.0, 1.0).to_linear();
    let mixed = bevy::color::LinearRgba {
        red: c.red + (w.red - c.red) * amount,
        green: c.green + (w.green - c.green) * amount,
        blue: c.blue + (w.blue - c.blue) * amount,
        alpha: c.alpha,
    };
    mixed.into()
}

fn unused_scaled(color: Color, factor: f32) -> Color {
    let mut l = color.to_linear();
    l.red = (l.red * factor).clamp(0.0, 1.0);
    l.green = (l.green * factor).clamp(0.0, 1.0);
    l.blue = (l.blue * factor).clamp(0.0, 1.0);
    l.into()
}

fn highlight_for(
    orig: &Handle<StandardMaterial>,
    cache: &mut HighlightCache,
    materials: &mut Assets<StandardMaterial>,
    variant: i32,
) -> Handle<StandardMaterial> {
    let key = HighlightKey {
        handle: orig.clone(),
        variant,
    };
    if let Some(h) = cache.0.get(&key) {
        return h.clone();
    }

    let mut mat = materials
        .get(orig)
        .cloned()
        .unwrap_or_else(StandardMaterial::default);

    // Brighter regardless of texture
    // mat.base_color = lighten_towards_white(mat.base_color, 0.22);

    // Visible variants (unchanged: 1 = legal-move red-ish, 2 = last-move/piece blue-ish)
    if variant == 1 {
        mat.base_color = Color::from(RED_300);
        mat.emissive = LinearRgba::rgb(0.9, 0.0, 0.0);
    } else {
        mat.base_color = Color::from(BLUE_300);
        // no emissive for this variant
    }
    // (Optionally bring back this emissive for extra pop:)
    // mat.emissive = scaled(mat.base_color, 0.10).into();

    let hi = materials.add(mat);
    cache.0.insert(key, hi.clone());
    hi
}



/*

// Old
commands.add_observer(|trigger: Trigger<OnAdd, Player>| {
    info!("Spawned player {}", trigger.target());
});

// New
commands.add_observer(|add: On<Add, Player>| {
    info!("Spawned player {}", add.entity);
});

*/





fn swap_to_highlight_on<E: bevy::prelude::EntityEvent>(
    variant: i32,
) -> impl Fn(
    On<E>,
    ResMut<HighlightCache>,
    ResMut<Assets<StandardMaterial>>,
    Query<&mut MeshMaterial3d<StandardMaterial>>,
    Query<&OriginalMaterial>,
) {
    move |trigger, mut cache, mut materials, mut mut_mat_q, orig_q| {
        if let (Ok(mut mat), Ok(orig)) = (
            mut_mat_q.get_mut(trigger.event_target()), // ****
            orig_q.get(trigger.event_target()),
        ) {
            let hi = highlight_for(&orig.0, &mut cache, &mut materials, variant);
            mat.0 = hi;
        }
    }
}

//fn swap_back_on<E>()
fn swap_back_on<E: bevy::prelude::EntityEvent>()
-> impl Fn(On<E>, Query<&mut MeshMaterial3d<StandardMaterial>>, Query<&OriginalMaterial>) {
    move |trigger, mut mut_mat_q, orig_q| {
        if let (Ok(mut mat), Ok(orig)) = (
            mut_mat_q.get_mut(trigger.event_target()),
            orig_q.get(trigger.event_target()),
        ) {
            mat.0 = orig.0.clone();
        }
    }
}

// === Helpers ===
#[inline]
fn idx_to_vec3(idx: i8) -> Vec3 {
    let x = (7 - idx / 8) as f32;
    let z = (idx % 8) as f32;
    Vec3::new(x, 0.0, z)
}

#[inline]
fn vec3_to_idx(v: Vec3) -> i8 {
    (7 - v.x as i8) * 8 + v.z as i8
}

// === App ===
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy 3D-Chess".into(),
                name: Some("bevy.app".into()),
                ..default()
            }),
            ..default()
        }))
        .register_type::<PositionData>()
        .add_plugins(MeshPickingPlugin)
        .add_plugins(PanOrbitCameraPlugin)
        .insert_resource(NextMoveTask(None))
        .insert_resource(SelectionState::default())
        .insert_resource(Figure::default())
        .insert_resource(SecsPerMove::default())
        .insert_resource(Txt::default())
        .insert_resource(State::Playing)
        .insert_resource(GameData::default())
        .insert_resource(EnginePlays::default())
        .insert_resource(HighlightCache::default())
        .insert_resource(TaggedCache::default())
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_menu_text)
        .add_systems(Update, move_figures)
        .add_systems(Update, new_game)
        .add_systems(Update, engine)
        .add_systems(Update, text_update_system)
        .add_systems(Update, time_text_update_system)
        .add_systems(Update, turn_text_update_system)
        .add_systems(Update, nxt_text_update_system)
        .add_systems(Update, keyboard_input_system)
        .add_systems(Update, do_engine_move)
        // Apply tag-based highlights to squares
        .add_systems(Update, update_square_highlights)
        .run();
}

// === Input ===
fn keyboard_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<Txt>,
    mut sides: ResMut<EnginePlays>,
    mut secs: ResMut<SecsPerMove>,
    game_data: ResMut<GameData>,
) {
    // Time adjust
    let old = secs.time;
    if keyboard.pressed(KeyCode::NumpadAdd) {
        secs.time += 0.05;
    }
    if keyboard.pressed(KeyCode::NumpadSubtract) {
        secs.time -= 0.05;
    }
    if (secs.time - old).abs() > f32::EPSILON {
        secs.time = secs.time.clamp(0.3, 5.0);
        ui.time = format!("Secs per move: {:.1}", secs.time);
    }

    // Toggle sides
    let mut changed = false;
    if keyboard.just_pressed(KeyCode::Numpad1) {
        sides.t[0] = !sides.t[0];
        changed = true;
    } else if keyboard.just_pressed(KeyCode::Numpad2) {
        sides.t[1] = !sides.t[1];
        changed = true;
    }
    if changed {
        ui.turn = format!(
            "{} (1) vs {} (2)",
            PLAYER_LABEL[sides.t[0] as usize], PLAYER_LABEL[sides.t[1] as usize]
        );
    }

    // Engine debug: print move list
    if keyboard.just_pressed(KeyCode::KeyM) {
        engine::print_move_list(&game_data.game.lock().unwrap());
    }
}

fn new_game(
    mut commands: Commands,
    pieces: Query<(Entity, &mut Figure)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<Txt>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut game_data: ResMut<GameData>,
) {
    if keyboard.pressed(KeyCode::Numpad0) {
        clear_board(&mut commands, pieces);
        engine::reset_game(&mut game_data.game.lock().unwrap());
        // clear tags
        game_data.tagged = [0; 64];
        populate_board(&mut commands, &asset_server, &mut materials, &mut game_data);
        ui.ui_text = "New game".to_string();
        ui.nxt = "White starts the game".to_string();
    }
}

// === UI Text markers ===
#[derive(Component)]
struct TimeInfoText;
#[derive(Component)]
struct NxtInfoText;
#[derive(Component)]
struct TurnInfoText;
#[derive(Component)]
struct InfoText;

fn setup_menu_text(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Sidebar info
    commands
        .spawn((
            Text::new("Help text\n"),
            TextFont {
                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                font_size: 32.0,
                ..default()
            },
            InfoText,
        ))
        .with_child((
            TextSpan::new("Player info\n"),
            (
                TextFont {
                    #[cfg(not(feature = "default_font"))]
                    font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                    font_size: 24.0,
                    ..default()
                },
                TextColor(GOLD.into()),
            ),
            TurnInfoText,
        ))
        .with_child((
            TextSpan::new("Time info\n"),
            (
                TextFont {
                    #[cfg(not(feature = "default_font"))]
                    font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                    font_size: 24.0,
                    ..default()
                },
                TextColor(GOLD.into()),
            ),
            TimeInfoText,
        ))
        .with_child((
            TextSpan::new("Next move\n"),
            (
                TextFont {
                    #[cfg(not(feature = "default_font"))]
                    font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                    font_size: 24.0,
                    ..default()
                },
                TextColor(GOLD.into()),
            ),
            NxtInfoText,
        ));
}

fn time_text_update_system(t: Res<Txt>, mut q: Query<&mut TextSpan, With<TimeInfoText>>) {
    for mut span in &mut q {
        **span = format!("{}\n", t.time);
    }
}

fn turn_text_update_system(t: Res<Txt>, mut q: Query<&mut TextSpan, With<TurnInfoText>>) {
    for mut span in &mut q {
        **span = format!("{}\n", t.turn);
    }
}

fn nxt_text_update_system(t: Res<Txt>, mut q: Query<&mut TextSpan, With<NxtInfoText>>) {
    for mut span in &mut q {
        **span = format!("{}\n", t.nxt);
    }
}

fn text_update_system(t: Res<Txt>, mut q: Query<&mut Text, With<InfoText>>) {
    for mut text in &mut q {
        **text = format!("{}\n", t.ui_text);
    }
}

// === Scene management ===
fn clear_board(commands: &mut Commands, mut pieces: Query<(Entity, &mut Figure)>) {
    for (entity, _) in pieces.iter_mut() {
        commands.entity(entity).despawn();
    }
}

fn do_engine_move(
    mut pieces: Query<(Entity, &mut Figure)>,
    mut game_data: ResMut<GameData>,
    mut ui: ResMut<Txt>,
    secs: Res<SecsPerMove>,
    sides: Res<EnginePlays>,
    mut state: ResMut<State>,
    mut commands: Commands,
    mut task: ResMut<NextMoveTask>,
    mut pos_q: Query<&mut PositionData>,
) {
    if let Some(ref mut next_move_task) = task.0 {
        if let Some(m) = future::block_on(future::poll_once(next_move_task)) {
            // Early game over
            if m.state == engine::STATE_CHECKMATE {
                ui.ui_text.push_str(" Checkmate, game terminated!");
                ui.nxt.clear();
                *state = State::GameTerminated;
                task.0 = None;
                return;
            }

            // Tag last move squares (for optional visualization)
            game_data.tagged = [0; 64];
            game_data.tagged[m.src as usize] = TAG_LAST;
            game_data.tagged[m.dst as usize] = TAG_LAST;

            // Update UI
            {
                let game = game_data.game.lock().unwrap();
                ui.refresh(secs.time, &sides, &game);
            }

            let flag = engine::do_move(
                &mut game_data.game.lock().unwrap(),
                m.src as i8,
                m.dst as i8,
                false,
            );

            ui.ui_text = engine::move_to_str(
                & game_data.game.lock().unwrap(),
                m.src as i8,
                m.dst as i8,
                flag,
            ) + &format!(" (score: {})", m.score);

            if m.checkmate_in == 2 && m.score == engine::KING_VALUE as i64 {
                ui.ui_text.push_str(" Checkmate, game terminated!");
                ui.nxt.clear();
                *state = State::GameTerminated;
                task.0 = None;
            } else if m.score > engine::KING_VALUE_DIV_2 as i64
                || m.score < -engine::KING_VALUE_DIV_2 as i64
            {
                ui.ui_text.push_str(&format!(
                    " Checkmate in {}",
                    if m.score > 0 {
                        m.checkmate_in / 2 - 1
                    } else {
                        m.checkmate_in / 2 + 1
                    }
                ));
            }

            // Capture
            let dst = idx_to_vec3(m.dst as i8);
            for (entity, piece) in pieces.iter_mut() {
                if piece.location == dst {
                    commands.entity(entity).despawn();
                }
            }

            // Move piece
            let src = idx_to_vec3(m.src as i8);
            for (entity, mut piece) in pieces.iter_mut() {
                if piece.location == src {
                    piece.location = dst;
                    if let Ok(mut pos) = pos_q.get_mut(entity) {
                        pos.location = dst;
                    }
                }
            }

            task.0 = None;
            if *state != State::GameTerminated {
                *state = State::Playing;
            }
        }
    }
}

fn move_figures(mut q: Query<(&mut Transform, &mut Figure)>, time: Res<Time>) {
    const REACHED_EPS: f32 = 0.05; // stop threshold in world units
    const LIFT_SCALE: f32 = 0.7; // visual arc scale
    const LAND_STEP: f32 = 0.05; // per-frame landing drop

    let dt = time.delta_secs();
    let a = ACCELERATION;
    let v_max = MAX_SPEED;

    for (mut tf, mut fig) in &mut q {
        // Work in a y=0 plane without touching the transform yet
        let mut pos = tf.translation;
        pos.y = 0.0;

        let to_target = fig.location - pos;
        let dist = to_target.length();

        // Default: keep current height (used for the soft landing branch)
        let mut new_y = tf.translation.y;

        if dist > REACHED_EPS {
            // Move along the path using current speed
            let dir = to_target / dist; // safe: dist > 0
            let old_speed = fig.speed;
            pos += dir * (old_speed * dt);

            // Decide accel/decel from stopping distance
            let stopping_dist = (old_speed * old_speed) / (2.0 * a);
            if dist <= stopping_dist {
                // Decelerate: average of “explicit decel” and “recompute from distance”
                let v1 = old_speed - a * dt;
                let v2 = (2.0 * a * dist).sqrt();
                fig.speed = 0.5 * (v1 + v2);
            } else {
                // Accelerate
                fig.speed = old_speed + a * dt;
            }

            // Clamp and update vertical arc from average speed
            fig.speed = fig.speed.clamp(0.0, v_max);
            let v_avg = 0.5 * (old_speed + fig.speed);
            new_y = LIFT_SCALE * v_avg.sqrt();

            // Commit the planar move + arc
            tf.translation = Vec3::new(pos.x, new_y, pos.z);
        } else {
            // Close enough: soften the landing without popping
            new_y = (new_y - LAND_STEP).max(0.0);
            tf.translation.y = new_y;
        }
    }
}

fn engine(
    secs: Res<SecsPerMove>,
    mut task: ResMut<NextMoveTask>,
    sides: Res<EnginePlays>,
    mut state: ResMut<State>,
    game_data: ResMut<GameData>,
) {
    if *state == State::Playing {
        let next = game_data.game.lock().unwrap().move_counter as usize % 2;
        if sides.t[next] {
            *state = State::Waiting;
            game_data.game.lock().unwrap().secs_per_move = secs.time;
            if task.0.is_none() {
                let pool = AsyncComputeTaskPool::get();
                let game_clone = game_data.game.clone();
                let new_task =
                    pool.spawn(async move { engine::reply(&mut game_clone.lock().unwrap()) });
                task.0 = Some(new_task);
            }
        }
    }
}

// --- Click handling + legal-move tagging ---
fn process_mouse_click(
    mut click: On<Pointer<Click>>,        // observer param; provides the clicked entity
    mut selection: ResMut<SelectionState>,
    mut commands: Commands,
    secs: Res<SecsPerMove>,
    sides: Res<EnginePlays>,
    state: Res<State>,
    mut pieces: Query<(Entity, &mut Figure)>,
    mut game_data: ResMut<GameData>,
    mut ui: ResMut<Txt>,
    mut pos_q: Query<&mut PositionData>,
) {
    // Only allow interaction while game is "Playing"
    if *state != State::Playing {
        return;
    }

    // If it's the engine's turn, ignore clicks
    let next = game_data.game.lock().unwrap().move_counter as usize % 2;
    if sides.t[next] {
        return;
    }

    // The entity that received this click (i.e., the board square or piece you observed on)
    let target = click.event_target();

    // We only care about entities that have PositionData
    let Ok(pd) = pos_q.get(target) else { return; };
    let loc = pd.location;
    let pos = PositionData { location: loc };

    // === First click: select a piece (must be a square that currently holds a piece) ===
    if selection.first_selection.is_none() {
        // Verify there is a piece on that square
        let mut found_piece = false;
        for (_e, p) in pieces.iter_mut() {
            if p.location == loc {
                selection.first_selection = Some((target, pos));
                found_piece = true;
                break;
            }
        }

        // If a piece was selected, compute and tag all legal destination squares
        if found_piece {
            let a = vec3_to_idx(pos.location) as i64;
            let mut new_tags = [0i8; 64];

            {
                let mut g = game_data.game.lock().unwrap();
                for b in 0..64 {
                    if engine::move_is_valid2(&mut g, a, b as i64) {
                        new_tags[b] = TAG_LEGAL; // legal destination
                    }
                }
            }

            // Also tag the selected source square as "last"
            let ai = a as usize;
            if ai < 64 {
                new_tags[ai] = TAG_LAST;
            }

            game_data.tagged = new_tags;
        }

        return;
    }

    // === Second click: attempt to move to the clicked square ===
    let (_, first) = selection.first_selection.as_ref().unwrap();
    let a = vec3_to_idx(first.location) as i64;
    let b = vec3_to_idx(pos.location) as i64;

    if !engine::move_is_valid2(&mut game_data.game.lock().unwrap(), a, b) {
        ui.ui_text = "invalid move, ignored.".to_owned();
        selection.first_selection = None;
        game_data.tagged = [0; 64]; // clear highlights
        return;
    }

    ui.refresh(secs.time, &sides, &game_data.game.lock().unwrap());

    // Handle capture on destination square
    for (entity, mut piece) in pieces.iter_mut() {
        if piece.location == pos.location {
            commands.entity(entity).despawn();
        }
        if piece.location == first.location {
            piece.location = pos.location;
            if let Ok(mut pd) = pos_q.get_mut(entity) {
                pd.location = pos.location;
            }
        }
    }

    // Tag last move (source & destination)
    game_data.tagged = [0; 64];
    game_data.tagged[a as usize] = TAG_LAST;
    game_data.tagged[b as usize] = TAG_LAST;

    let flag = engine::do_move(&mut game_data.game.lock().unwrap(), a as i8, b as i8, false);
    ui.ui_text = engine::move_to_str(&game_data.game.lock().unwrap(), a as i8, b as i8, flag);

    // Reset selection after a completed move
    selection.first_selection = None;
}



// --- Squares ---
fn create_squares(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    _materials: &mut ResMut<Assets<StandardMaterial>>, // kept for parity with original signature
    meshes: &mut ResMut<Assets<Mesh>>,
) {
    let mat2: Handle<StandardMaterial> =
        asset_server.load("models/wooden_chess_board.glb#Material1");
    let mat1: Handle<StandardMaterial> =
        asset_server.load("models/wooden_chess_board.glb#Material0");

    for i in 0..8 {
        for j in 0..8 {
            let location = Vec3::new(i as f32, 0.0, j as f32);
            let mesh_handle = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 0.4)));
            let base_mat: Handle<StandardMaterial> = if (i + j) % 2 == 0 {
                mat1.clone()
            } else {
                mat2.clone()
            };
            // engine index mapping for this board coordinate:
            let idx = (7 - i) * 8 + j;

            commands
                .spawn((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(base_mat.clone()),
                    Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                        .with_translation(Vec3 {
                            x: i as f32,
                            y: -0.2,
                            z: j as f32,
                        }),
                    PositionData { location },
                    OriginalMaterial(base_mat.clone()),
                    Square { idx },
                ))
                // we can enable square hover here if desired:
                // .observe(swap_to_highlight_on::<Pointer<Over>>(SQUARE_HOVER_FACTOR))
                // .observe(swap_back_on::<Pointer<Out>>())
                .observe(process_mouse_click);
        }
    }
}

// Applies game_data.tagged to board square materials.
// If tagged[idx] != 0, swap to cached highlight; else revert to original.
fn update_square_highlights(
    mut cache: ResMut<HighlightCache>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    game_data: Res<GameData>,
    mut last: ResMut<TaggedCache>,
    mut q: Query<(
        &Square,
        &mut MeshMaterial3d<StandardMaterial>,
        &OriginalMaterial,
    )>,
) {
    // Early-out if nothing changed
    if game_data.tagged == last.0 {
        return;
    }

    for (sq, mut mat, orig) in q.iter_mut() {
        let tagged = game_data.tagged[sq.idx];

        if tagged == TAG_LAST {
            let hi = highlight_for(&orig.0, &mut cache, &mut mats, PIECE_HOVER_FACTOR);
            mat.0 = hi;
        } else if tagged != 0 {
            let hi = highlight_for(&orig.0, &mut cache, &mut mats, SQUARE_HOVER_FACTOR);
            mat.0 = hi;
        } else {
            mat.0 = orig.0.clone();
        }
    }

    last.0 = game_data.tagged;
}

fn populate_board(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    game_data: &mut ResMut<GameData>,
) {
    const PIECE_POS: [usize; 6] = [0, 2, 3, 5, 13, 14];
    const NUM_PIECES: usize = 12; // 6 white + 6 black variants

    // engine index -> glb mesh index (see original code)
    const ENGINE_TO_MODEL: [usize; 13] = [
        1,
        4,
        5,
        0,
        2,
        3,
        99,
        3 + 6,
        2 + 6,
        0 + 6,
        5 + 6,
        4 + 6,
        1 + 6,
    ];

    // Preload piece meshes (white then black set)
    let mut figures: Vec<Handle<Mesh>> = Vec::with_capacity(NUM_PIECES);
    for i in 0..(NUM_PIECES / 2) {
        figures.push(asset_server.load(format!(
            "models/wooden_chess_board.glb#Mesh{}/Primitive0",
            PIECE_POS[i]
        )));
    }
    for i in 0..(NUM_PIECES / 2) {
        figures.push(asset_server.load(format!(
            "models/wooden_chess_board.glb#Mesh{}/Primitive0",
            PIECE_POS[i] + 18
        )));
    }

    let mat2: Handle<StandardMaterial> =
        asset_server.load("models/wooden_chess_board.glb#Material1");
    let mat1: Handle<StandardMaterial> =
        asset_server.load("models/wooden_chess_board.glb#Material0");
    let _hover_mat = materials.add(Color::from(CYAN_300)); // unused (kept for reference)

    let engine_board = engine::get_board(&game_data.game.lock().unwrap());

    for i in 0..8 {
        for j in 0..8 {
            let h = engine_board[j + i * 8];
            if h == 0 {
                continue; // empty square
            }

            let rotation = if i > 5 {
                Quat::from_rotation_y(std::f32::consts::PI)
            } else {
                Quat::from_rotation_y(0.0)
            };
            let location = Vec3::new((7 - i) as f32, 0.0, j as f32);
            let base_mat = if h > 0 { mat2.clone() } else { mat1.clone() };
            let mesh_index = ENGINE_TO_MODEL[(h + 6) as usize];

            commands
                .spawn((
                    Mesh3d(figures[mesh_index].clone()),
                    MeshMaterial3d(base_mat.clone()),
                    Transform::from_translation(location)
                        .with_scale(Vec3::splat(1.0))
                        .with_rotation(rotation),
                    PositionData { location },
                    Figure {
                        location,
                        speed: 0.0,
                    },
                    OriginalMaterial(base_mat.clone()),
                ))
                .observe(swap_to_highlight_on::<Pointer<Over>>(PIECE_HOVER_FACTOR))
                .observe(swap_back_on::<Pointer<Out>>())
                .observe(process_mouse_click);
        }
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut game_data: ResMut<GameData>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    create_squares(&mut commands, &asset_server, &mut materials, &mut meshes);
    populate_board(&mut commands, &asset_server, &mut materials, &mut game_data);

    commands.spawn((
        PointLight {
            //intensity: 2000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    commands.spawn((
        Camera3d { ..default() },
        Transform::from_xyz(12., 7., 4.).looking_at(
            Vec3 {
                x: 4.,
                y: 0.,
                z: 4.,
            },
            Vec3::Y,
        ),
        PanOrbitCamera {
            button_orbit: MouseButton::Middle,
            focus: Vec3::new(4.0, 0.0, 4.0),
            ..default()
        },
    ));
}
// 957 lines
