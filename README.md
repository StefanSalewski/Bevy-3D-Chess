# Bevy 3D Chess

A Rust implementation of the salewski-chess engine, featuring a simple 3D user interface built with Bevy.

![Chess UI](http://ssalewski.de/tmp/Bevy-3D-Chess.png)

This Rust version of the chess engine includes several improvements and bug fixes over the original Nim version, and it eliminates the use of global variables.

### Graphics

The chess pieces were created using Blender by Bhargav Limje and are licensed under [Creative Commons Attribution](http://creativecommons.org/licenses/by/4.0/).
See the [Wooden Chess Board](https://skfb.ly/oXqwI).

### Features

- **User Interface**: A basic Bevy 3D interface allows you to set move time limits, choose players, and freely zoom and rotate the board.
- **Game Modes**: Supports both human vs. human gameplay and automatic engine-based games.
- **Move List**: When run from the terminal, you can press the 'm' key to print a list of moves, which may help with debugging the engine.
- **Non-blocking UI**: The chess engine runs on a background thread to keep the GUI responsive.

### Background

We initially planned to wait for the new Rust Xilem GUI to improve the interface. However, Xilem is still in its early stages, with limited documentation and examples, making it difficult to use.

To explore the complexity of creating games in Rust using Bevy, we developed a simple 3D interface for our chess engine. This project serves as an example of 3D graphics in Bevy and demonstrates the use of background worker threads.

### AI Assistance

Some parts of the user interface were developed with AI tools. GPT-4 was instrumental in helping us understand official Bevy examples and adapt older code to the latest version of Bevy.

### Current Status

The chess engine has undergone minimal testing so far, but it serves as a concise example of using Bevy for asset loading and background task execution.
The entire Bevy code is contained in a single `main.rs` file, with only 600 lines of code. The chess engine consists of approximately 2,000 lines in a single file. The project currently uses Bevy 0.14.2, but we may update it for newer Bevy versions in the future.

### Future Plans

We may develop a Xilem-based GUI by the end of the year or extend the current Bevy interface.

### How to Run

```sh
git clone https://github.com/stefansalewski/Bevy-3D-Chess.git
cd Bevy-3D-Chess
cargo run --release
```

---

For actual gameplay, you might prefer the 2D egui version with the same engine, available at [tiny-chess](https://github.com/StefanSalewski/tiny-chess).

Errors such as "WARN bevy_gltf::loader: Unknown vertex attribute TEXCOORD_3" that appear in the terminal are a known Bevy issue. Bevy may address this in future updates, or we may attempt to modify the Blender model to suppress the warnings.

---

[Text and layout optimized with assistance from GPT-4]

