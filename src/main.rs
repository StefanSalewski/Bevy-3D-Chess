//"Chess Scene Pieces / Blender" (https://skfb.ly/67OF8) by moyicat is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).

//"Realistic 3D Chess Pieces (Blender)" (https://skfb.ly/oXTUP) by ronildo.facanha is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).

//"Chess" (https://skfb.ly/6uVLu) by xnicrox is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).

//"Wooden Chess Board" (https://skfb.ly/oXqwI) by Bhargav Limje is licensed under Creative Commons Attribution (http://creativecommons.org/licenses/by/4.0/).

// Plain Bevy frontend for the tiny Salewski chess engine
// v 0.1 -- 08-OCT-2024
// (C) 2015 - 2032 Dr. Stefan Salewski
// All rights reserved.

use bevy::{
    prelude::*,
    tasks::{futures_lite::future, AsyncComputeTaskPool, Task},
};

use bevy_mod_picking::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use std::sync::{Arc, Mutex};

mod engine;

const DEFAULT_TIME_PER_MOVE: f32 = 2.0;

#[derive(Resource)]
struct NextMoveTask(Option<Task<engine::Move>>);

#[derive(Component, Reflect, Clone)]
struct PositionData {
    location: Vec3,
}

#[derive(Resource, Default, Component)]
struct Figure {
    location: Vec3,
    speed: f32,
}

// A unit struct to help identify the FPS UI component, since there may be many Text components
#[derive(Component)]
struct FpsText;

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
            time: format!("{} secs per move\n  use keypad + or - to modify", DEFAULT_TIME_PER_MOVE).to_string(),
            nxt: "White starts the game".to_string(),
        }
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

#[derive(Resource, Component)]
struct GameData {
    game: Arc<Mutex<engine::Game>>,
    rotated: bool,         // unused!
    tagged: engine::Board, // unused, as we can not mark squares with Bevy
}

impl Default for GameData {
    fn default() -> Self {
        Self {
            game: Arc::new(Mutex::new(engine::new_game())),
            rotated: true,
            tagged: [0; 64],
        }
    }
}

#[derive(Resource, Default)]
struct SelectionState {
    first_selection: Option<(Entity, PositionData)>,
}

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
        .add_plugins(DefaultPickingPlugins)
        .add_plugins(PanOrbitCameraPlugin)
        .insert_resource(NextMoveTask(None))
        .insert_resource(SelectionState::default())
        .insert_resource(Figure::default())
        .insert_resource(SecsPerMove::default())
        .insert_resource(Txt::default())
        .insert_resource(State::Playing)
        .insert_resource(GameData::default())
        .insert_resource(EnginePlays::default())
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_menu_text)
        .add_systems(Update, move_figures)
        .add_systems(Update, new_game)
        .add_systems(Update, engine)
        .add_systems(Update, text_update_system)
        .add_systems(Update, keyboard_input_system)
        .add_systems(Update, do_engine_move)
        .run();
}

const MAP: [&str; 2] = ["Human", "Computer"];

fn keyboard_input_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut t: ResMut<Txt>,
    mut ep: ResMut<EnginePlays>,
    mut time: ResMut<SecsPerMove>,
    game_data: ResMut<GameData>,
) {
    let old_time = time.time;
    if keyboard_input.pressed(KeyCode::NumpadAdd) {
        time.time += 0.05;
    }
    if keyboard_input.pressed(KeyCode::NumpadSubtract) {
        time.time -= 0.05;
    }
    if time.time != old_time {
        time.time = time.time.clamp(0.3, 5.0);
        t.time = format!("Secs per move: {:.1}", time.time);
    }
    let mut b = false;
    if keyboard_input.just_pressed(KeyCode::Numpad1) {
        ep.t[0] = !ep.t[0];
    } else if keyboard_input.just_pressed(KeyCode::Numpad2) {
        ep.t[1] = !ep.t[1];
    } else {
        b = true;
    }
    if !b {
        t.turn = format!(
            "{} (1) vs {} (2)",
            MAP[ep.t[0] as usize], MAP[ep.t[1] as usize]
        );
    }
    if keyboard_input.just_pressed(KeyCode::KeyM) {
        engine::print_move_list(&game_data.game.lock().unwrap()); // for engine debugging purpose
    }
}

fn new_game(
    mut commands: Commands,
    pieces_query: Query<(Entity, &mut Figure)>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut t: ResMut<Txt>,
    asset_server: Res<AssetServer>,
    mut game_data: ResMut<GameData>,
) {
    if keyboard_input.pressed(KeyCode::Numpad0) {
        clear_board(&mut commands, pieces_query);
        engine::reset_game(&mut game_data.game.lock().unwrap());
        populate_board(&mut commands, &asset_server, &mut game_data);
        t.ui_text = "New game".to_string();
        t.nxt = "White starts the game".to_string();
    }
}

fn setup_menu_text(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_sections([
            TextSection::from_style(TextStyle {
                font_size: 32.0,
                ..default()
            }),
            TextSection::from_style(TextStyle {
                font_size: 24.0,
                color: Color::srgb(0.7, 0.7, 0.7),
                ..default()
            }),
            TextSection::from_style(TextStyle {
                font_size: 24.0,
                color: Color::srgb(0.7, 0.7, 0.7),
                ..default()
            }),
            TextSection::from_style(TextStyle {
                font_size: 24.0,
                color: Color::srgb(0.7, 0.7, 0.7),
                ..default()
            }),
        ]),
        FpsText,
    ));
}

fn text_update_system(t: Res<Txt>, mut query: Query<&mut Text, With<FpsText>>) {
    for mut text in &mut query {
        text.sections[0].value = format!("{}\n", t.ui_text);
        text.sections[1].value = format!("{}\n", t.turn);
        text.sections[2].value = format!("{}\n", t.time);
        text.sections[3].value = format!("{}\n", t.nxt);
    }
}

fn clear_board(commands: &mut Commands, mut pieces_query: Query<(Entity, &mut Figure)>) {
    for (piece_ent, _p) in pieces_query.iter_mut() {
        commands.entity(piece_ent).despawn();
    }
}

fn do_engine_move(
    mut pieces_query: Query<(Entity, &mut Figure)>,
    mut game_data: ResMut<GameData>,
    mut t: ResMut<Txt>,
    time: Res<SecsPerMove>,
    ep: Res<EnginePlays>,
    mut state: ResMut<State>,
    mut commands: Commands,
    mut task: ResMut<NextMoveTask>,
    mut position_data_query: Query<&mut PositionData>,
) {
    if let Some(ref mut next_move_task) = task.0 {
        if let Some(m) = future::block_on(future::poll_once(next_move_task)) {
            game_data.tagged = [0; 64];
            game_data.tagged[m.src as usize] = 2;
            game_data.tagged[m.dst as usize] = 2;
            if game_data.rotated {
                game_data.tagged.reverse();
            }
            t.turn = format!(
                "{} (1) vs {} (2)",
                MAP[ep.t[0] as usize], MAP[ep.t[1] as usize]
            );
            t.time = format!("Secs per move: {:.1}", time.time);
            let next = game_data.game.lock().unwrap().move_counter as usize % 2;
            t.nxt = format!("Next move: {}", ["Black", "White"][next]);
            let flag = engine::do_move(
                &mut game_data.game.lock().unwrap(),
                m.src as i8,
                m.dst as i8,
                false,
            );
            t.ui_text = engine::move_to_str(
                &mut game_data.game.lock().unwrap(),
                m.src as i8,
                m.dst as i8,
                flag,
            ) + &format!(" (score: {})", m.score);
            if m.score == engine::KING_VALUE as i64 {
                t.ui_text.push_str(" Checkmate, game terminated!");
                t.nxt.clear();
                *state = State::GameTerminated;
                return;
            } else if m.score > engine::KING_VALUE_DIV_2 as i64 {
                t.ui_text.push_str(&format!(
                    " Checkmate in {}",
                    (engine::KING_VALUE as i64 - m.score) / 2
                ));
            }
            let x = (7 - m.dst / 8) as f32;
            let y = (m.dst % 8) as f32;
            let dst = Vec3::new(x, 0.0, y);
            for (piece_ent, p) in pieces_query.iter_mut() {
                if p.location == dst {
                    commands.entity(piece_ent).despawn();
                }
            }
            let x = (7 - m.src / 8) as f32;
            let y = (m.src % 8) as f32;
            let src = Vec3::new(x, 0.0, y);
            for (_piece_entity, mut piece) in pieces_query.iter_mut() {
                if piece.location == src {
                    piece.location = dst;
                    let pos = position_data_query.get_mut(_piece_entity);
                    pos.unwrap().location = dst;
                }
            }
            task.0 = None;
            *state = State::Playing;
        }
    }
}

fn move_figures(mut figures: Query<(&mut Transform, &mut Figure)>, timer: Res<Time>) {
    for (mut transform, mut figure) in &mut figures {
        let g = 1.0; // acceleration
        let vmax = 3.0; // maximum speed
        transform.translation.y = 0.0;
        let dist = figure.location - transform.translation;
        if dist.length() > 0.05 || figure.speed > 0.05 {
            transform.translation += dist.normalize() * timer.delta_seconds() * figure.speed;
            let stop_dist = (figure.speed * figure.speed) / (2.0 * g);
            if dist.length() <= stop_dist {
                //figure.speed -= timer.delta_seconds() * g; // not really smooth
                figure.speed = (dist.length() * 2.0 * g).sqrt();
            } else {
                figure.speed += timer.delta_seconds() * g;
            }
            figure.speed = figure.speed.clamp(0.0, vmax);
            transform.translation.y = figure.speed.sqrt(); // sqrt() gives a nice trajectory
        }
    }
}

fn engine(
    time: Res<SecsPerMove>,
    mut task: ResMut<NextMoveTask>,
    ep: Res<EnginePlays>,
    mut state: ResMut<State>,
    game_data: ResMut<GameData>,
) {
    if *state == State::Playing {
        let next = game_data.game.lock().unwrap().move_counter as usize % 2;
        if ep.t[next] {
            *state = State::Waiting;
            game_data.game.lock().unwrap().secs_per_move = time.time;
            if task.0.is_none() {
                let task_pool = AsyncComputeTaskPool::get();
                let game_clone = game_data.game.clone();
                let new_task = task_pool.spawn(async move {
                    let m = engine::reply(&mut game_clone.lock().unwrap());
                    m
                });
                task.0 = Some(new_task);
            }
        }
    }
}

fn process_mouse_click(
    mut selection_state: ResMut<SelectionState>,
    mut pointer_events: EventReader<Pointer<Click>>,
    mut commands: Commands,
    time: Res<SecsPerMove>,
    ep: Res<EnginePlays>,
    state: ResMut<State>,
    mut pieces_query: Query<(Entity, &mut Figure)>,
    game_data: ResMut<GameData>,
    mut t: ResMut<Txt>,
    mut position_data_query: Query<&mut PositionData>,
) {
    if *state == State::Playing {
        let next = game_data.game.lock().unwrap().move_counter as usize % 2;
        if !ep.t[next] {
            for click in pointer_events.read() {
                if position_data_query.get(click.target).is_ok() {
                    let loc = (position_data_query.get(click.target)).unwrap().location;
                    let position_data = PositionData { location: loc };
                    if selection_state.first_selection.is_none() {
                        for (_piece_ent, p) in pieces_query.iter_mut() {
                            if p.location == loc {
                                selection_state.first_selection =
                                    Some((click.target, position_data.clone()));
                            }
                        }
                    } else {
                        let (_, first_position_data) =
                            selection_state.first_selection.as_ref().unwrap();
                        let a = (7 - first_position_data.location[0] as i8) * 8
                            + first_position_data.location[2] as i8;
                        let b = (7 - position_data.location[0] as i8) * 8
                            + position_data.location[2] as i8;
                        if !engine::move_is_valid2(
                            &mut game_data.game.lock().unwrap(),
                            a as i64,
                            b as i64,
                        ) {
                            t.ui_text = "invalid move, ignored.".to_owned();
                            selection_state.first_selection = None;
                            return;
                        }
                        t.turn = format!(
                            "{} (1) vs {} (2)",
                            MAP[ep.t[0] as usize], MAP[ep.t[1] as usize]
                        );
                        t.time = format!("Secs per move: {:.1}", time.time);
                        t.nxt = format!("Next move: {}", ["Black", "White"][next]);
                        for (piece_entity, mut piece) in pieces_query.iter_mut() {
                            if piece.location == position_data.location {
                                commands.entity(piece_entity).despawn();
                            }
                            if piece.location == first_position_data.location {
                                piece.location = position_data.location;
                                let pos = position_data_query.get_mut(piece_entity);
                                pos.unwrap().location = position_data.location;
                            }
                        }
                        let flag =
                            engine::do_move(&mut game_data.game.lock().unwrap(), a, b, false);
                        t.ui_text =
                            engine::move_to_str(&mut game_data.game.lock().unwrap(), a, b, flag);
                        selection_state.first_selection = None;
                    }
                }
            }
        }
    }
}

fn create_squares(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    meshes: &mut ResMut<Assets<Mesh>>,
) {
    let mat_handle2 = asset_server.load("models/wooden_chess_board.glb#Material0");
    let mat_handle1 = asset_server.load("models/wooden_chess_board.glb#Material1");
    for i in 0..8 {
        for j in 0..8 {
            let location = Vec3::new(i as f32, 0.0, j as f32);
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(1.0, 1.0, 0.4)),
                    material: if (i + j) % 2 == 0 {
                        mat_handle2.clone()
                    } else {
                        mat_handle1.clone()
                    },
                    transform: Transform::from_rotation(Quat::from_rotation_x(
                        -std::f32::consts::FRAC_PI_2,
                    ))
                    .with_translation(Vec3 {
                        x: i as f32,
                        y: -0.2,
                        z: j as f32,
                    }),
                    ..default()
                },
                PositionData { location },
                PickableBundle::default(),
                On::<Pointer<Click>>::run(process_mouse_click),
            ));
        }
    }
}

fn populate_board(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    game_data: &mut ResMut<GameData>,
) {
    const PIECE_POS: [usize; 6] = [0_usize, 2, 3, 5, 13, 14];
    const BISHOP: usize = 0;
    const KING: usize = 1;
    const KNIGHT: usize = 2;
    const PAWN: usize = 3;
    const QUEEN: usize = 4;
    const ROOK: usize = 5;
    const ENGINE_TO_MODEL: [usize; 13] = [
        1,
        4,
        5,
        0,
        2,
        3,
        99, // dummy value for void position
        3 + 6,
        2 + 6,
        0 + 6,
        5 + 6,
        4 + 6,
        1 + 6,
    ]; // we can take the piece position from the engine
    let mut content = [6_usize; 64]; // or generate piece position from scratch
    content[0..8].copy_from_slice(&[ROOK, KNIGHT, BISHOP, KING, QUEEN, BISHOP, KNIGHT, ROOK]);
    content[8..16].copy_from_slice(&[PAWN; 8]);
    content[56..64].copy_from_slice(&[
        ROOK + 6,
        KNIGHT + 6,
        BISHOP + 6,
        KING + 6,
        QUEEN + 6,
        BISHOP + 6,
        KNIGHT + 6,
        ROOK + 6,
    ]);
    content[48..56].copy_from_slice(&[PAWN + 6; 8]);
    let engine_board = engine::get_board(&game_data.game.lock().unwrap());
    const NUM_PIECES: usize = 12;
    let mut figures: Vec<Handle<Mesh>> = Vec::new();
    for i in 0..(NUM_PIECES / 2) {
        let mesh_handle = asset_server.load(format!(
            "models/wooden_chess_board.glb#Mesh{}/Primitive0",
            PIECE_POS[i]
        ));
        figures.push(mesh_handle);
    }
    for i in 0..(NUM_PIECES / 2) {
        let mesh_handle = asset_server.load(format!(
            //"models/kkk/scene.gltf#Mesh{}/Primitive0", // load from the gltf textfile
            "models/wooden_chess_board.glb#Mesh{}/Primitive0",
            PIECE_POS[i] + 18
        ));
        figures.push(mesh_handle);
    }
    let mat_handle1 = asset_server.load("models/wooden_chess_board.glb#Material1");
    let mat_handle2 = asset_server.load("models/wooden_chess_board.glb#Material0");
    for i in 0..8 {
        for j in 0..8 {
            let math = if i >= 2 {
                mat_handle1.clone()
            } else {
                mat_handle2.clone()
            };
            if !(2..=5).contains(&i) {
                let rotation = if i < 2 {
                    Quat::from_rotation_y(std::f32::consts::PI)
                } else {
                    Quat::from_rotation_y(std::f32::consts::PI * 0.0)
                };
                let location = Vec3::new(i as f32, 0.0, j as f32);
                commands.spawn((
                    PbrBundle {
                        // !!! mesh: figures[content[j + i * 8]].clone(), // use our own position data
                        mesh: figures[ENGINE_TO_MODEL[(engine_board[j + i * 8] + 6) as usize]]
                            .clone(), // take the piece position from the engine
                        material: math.clone(),
                        transform: Transform::from_translation(location)
                            .with_scale(Vec3::splat(1.0))
                            .with_rotation(rotation),
                        ..default()
                    },
                    PositionData { location },
                    Figure {
                        location,
                        speed: 0.0,
                    },
                    PickableBundle::default(),
                    On::<Pointer<Click>>::run(process_mouse_click),
                ));
            }
        }
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut game_data: ResMut<GameData>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    create_squares(&mut commands, &asset_server, &mut meshes);
    populate_board(&mut commands, &asset_server, &mut game_data);
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(12., 7., 4.).looking_at(
                Vec3 {
                    x: 4.,
                    y: 0.,
                    z: 4.,
                },
                Vec3::Y,
            ),
            ..default()
        },
        PanOrbitCamera {
            button_orbit: MouseButton::Middle,
            focus: Vec3::new(4.0, 0.0, 4.0),
            ..default()
        },
    ));
}
// 606 lines
