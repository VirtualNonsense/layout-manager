# layout-manager

A work-in-progress [`cargo generate`] template for building **event-driven, component-based terminal UIs** with [Ratatui] and [Tokio].

The repository is both a working demo application and the blueprint for the template. Run it to see the architecture in action; read the source to understand the conventions a generated project will follow.

[`cargo generate`]: https://github.com/cargo-generate/cargo-generate
[Ratatui]: https://ratatui.rs
[Tokio]: https://tokio.rs

## Quick start

```sh
cargo run
```

The demo renders a two-pane layout. Default keybindings:

| Key | Action |
|---|---|
| `q` / `Esc` / `Ctrl-C` | Quit |
| `Tab` / `Shift-Tab` | Cycle focus |
| `Alt+Arrow` | Move focus geometrically |
| `↑` / `↓` | Navigate sidebar / change counter |
| Mouse click | Focus + sidebar item selection |
| Mouse scroll | Navigate sidebar / change counter |

## Architecture

The codebase is organised into three layers that communicate through typed events and commands.

```
┌─────────────────────────────────────────────┐
│  main.rs  —  terminal setup / teardown      │
│                                             │
│  App  ──────────────────────────────────┐  │
│    EventHandler  (Tokio async channel)  │  │
│    Ui  ──────────────────────────────┐  │  │
│      LayoutSpec  (rect tree)         │  │  │
│      FocusManager                    │  │  │
│      InputManager  (keymaps)         │  │  │
│      ComponentRegistry               │  │  │
│        SidebarComponent              │  │  │
│        ContentComponent              │  │  │
│    └─────────────────────────────────┘  │  │
│  └──────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

### Event flow

1. `EventTask` (background Tokio task) polls crossterm and a 30 FPS tick timer, pushing `EventContainer` values onto an unbounded mpsc channel.
2. `App::run()` receives each `EventContainer` and dispatches it.
3. Key / mouse events go to `Ui`, which resolves them through `InputManager` into a `Command`.
4. `Command` is one of:
   - `Command::App` — bubbled back to `App` as a `UiAction` (e.g. `Quit`).
   - `Command::Focus` — handled directly by `FocusManager` (cycle, geometric move).
   - `Command::Component` — forwarded to the currently focused (or pointer-hovered) component via `ComponentRegistry`.
5. A component's `on()` handler returns `EventOutcome::Consumed(actions)` or `EventOutcome::Ignored`. Consumed actions flow back to `App`.

### Layout

`LayoutSpec` is a recursive tree of `Leaf` and `Split` nodes. Calling `compute(area)` walks the tree and produces a flat `Vec<LaidOutRegion>`, each carrying the component ID, focus ID, and the final `Rect`.

A `Leaf` holds two IDs — `component` and `focus` — which are the same in the common case (`LayoutSpec::leaf(id)`). They can differ via `LayoutSpec::focused_leaf(component, focus)`, enabling a single component to occupy multiple layout slots with independent focus states (e.g. tabs or split-pane views).

### Component model

Implement the `Component` trait to create a new component:

```rust
pub trait Component {
    fn id(&self) -> ComponentId;
    fn kind() -> ComponentKind where Self: Sized;  // static string, used for input routing
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: RenderContext<'_>);
    fn on(&mut self, event: Box<dyn Event>) -> EventOutcome;
}
```

Components are registered with `UiBuilder::component()` and placed in the layout tree. `UiBuilder::build()` validates that every layout leaf references a registered component, that all focus IDs are unique, and that no registered component goes unused.

### Defining component events

Use the `new_event!` macro to declare typed events without boilerplate:

```rust
new_event!(MoveEvent, Direction2D);  // tuple struct
new_event!(Submit);                  // unit struct
new_event!(Resize { width: u16, height: u16 });  // named fields
new_event!(enum ScrollDir { Up, Down });          // enum
```

The macro derives `Clone`, `Debug`, and the `Event` trait (which enables type-erased dispatch and safe downcasting via `event.downcast_ref::<T>()`).

### Input routing

`InputManager` holds two lookup tables — global and per-`ComponentKind` — for both key and pointer bindings. `resolve_key()` and `resolve_pointer()` check component bindings first, then fall back to the global table.

`PointerBinding` has two forms:
- `Fixed(event)` — always produces the same event regardless of pointer position.
- `WithEvent` — wraps the full `PointerEvent` (including local coordinates relative to the component rect) in a `MouseEvent` and forwards it.

## Known limitations

- **`ComponentEvent` escalation is not yet implemented.** `EventContainer::ComponentEvent` exists to allow components to send arbitrary events back to `App` (beyond the `UiAction` enum), but the handler in `App::run()` is currently `todo!()`.
- **Component bindings are hard-coded in `InputManager::default_keymap()`.** The `register_component_bindings()` API exists for components to declare their own bindings, but it is not yet wired into the builder. For now, add component-specific bindings directly in `default_keymap()`.

## License


This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

[LICENSE]: ./LICENSE
