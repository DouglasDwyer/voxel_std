use voxel_engine::*;
use voxel_engine::input::*;
use voxel_engine::math::*;
use voxel_engine::physics::*;
use voxel_engine::player::*;
use voxel_engine::timing::*;
use wings::*;

instantiate_systems!(Client, [PlayerController]);

/// Implements a basic first-person character controller for movement.
#[export_system]
pub struct PlayerController {
    /// The context handle.
    ctx: WingsContextHandle<Self>,
    /// Holds handles for accessing user input.
    user_actions: UserActions,
}

impl PlayerController {
    fn print_hit_object(&mut self, transform: Transform) {
        let input = self.ctx.get::<dyn Input>();
        if let Some(direction) = input.pointer_direction() {
            let raycaster = self.ctx.get::<dyn Raycaster>();
            if let Some(hit) = raycaster.cast(&Ray { direction, position: transform.position, max_distance: f32::MAX }) {
                println!("HIT {hit:?}");
            }
        }
    }

    /// Moves the player according to user inputs.
    fn move_player(&mut self, _: &voxel_engine::timing::on::Frame) {
        let delta_time = self.ctx.get::<dyn FrameTiming>().frame_duration().as_secs_f32();
        let mut input = self.ctx.get_mut::<dyn Input>();
        let pointer_delta = input.pointer_delta();
        let look_vertical = input.get(self.user_actions.look_vertical);
        let look_horizontal = input.get(self.user_actions.look_horizontal);
        let jump = input.get(self.user_actions.jump);
        let move_forward = input.get(self.user_actions.move_forward);
        let move_sideways = input.get(self.user_actions.move_sideways);
        let sneak = input.get(self.user_actions.sneak);
        let toggle_pointer_lock = input.get(self.user_actions.toggle_pointer_lock);

        let lock_pointer = input.pointer_locked() && input.focused();
        input.set_pointer_locked(lock_pointer ^ toggle_pointer_lock.pressed);
        
        drop(input);

        let mut player = self.ctx.get_mut::<dyn Player>();
        let mut transform = player.get_transform();

        Self::update_player_look_direction(&mut transform, pointer_delta, delta_time * vec2(look_horizontal, look_vertical));

        let net_vertical_motion = if jump.held { 1.0 } else { 0.0 } + if sneak.held { -1.0 } else { 0.0 };
        Self::update_player_position(&mut transform, delta_time, vec3a(move_sideways, net_vertical_motion, move_forward));
        
        player.set_transform(transform);
        drop(player);
        self.print_hit_object(transform);
    }

    /// Registers the set of actions relevant to player movement.
    fn get_user_actions(ctx: &mut WingsContextHandle<Self>) -> UserActions {
        let mut input = ctx.get_mut::<dyn Input>();

        let look_vertical = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Look vertical"),
            "Causes the player to look up or down.",
            &[
                AnalogBinding {
                    invert: false,
                    raw_input: RawInput::GamepadAxis(GamepadAxis::RightStickY)
                },
            ]
        ));

        let look_horizontal = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Look horizontal"),
            "Causes the player to look left or right.",
            &[
                AnalogBinding {
                    invert: false,
                    raw_input: RawInput::GamepadAxis(GamepadAxis::RightStickX)
                },
            ]
        ));

        let jump = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Jump"),
            "Causes the player to jump or move upward.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::GamepadButton(GamepadButton::South)
                },
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::Key(Key::Space)
                },
            ]
        ));

        let move_forward = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Move forward"),
            "Causes the player to walk forward or backward.",
            &[
                AnalogBinding {
                    invert: false,
                    raw_input: RawInput::Key(Key::W)
                },
                AnalogBinding {
                    invert: true,
                    raw_input: RawInput::Key(Key::S)
                },
                AnalogBinding {
                    invert: false,
                    raw_input: RawInput::GamepadAxis(GamepadAxis::LeftStickY)
                },
            ]
        ));

        let move_sideways = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Move sideways"),
            "Causes the player to walk left or right.",
            &[
                AnalogBinding {
                    invert: false,
                    raw_input: RawInput::Key(Key::D)
                },
                AnalogBinding {
                    invert: true,
                    raw_input: RawInput::Key(Key::A)
                },
                AnalogBinding {
                    invert: false,
                    raw_input: RawInput::GamepadAxis(GamepadAxis::LeftStickX)
                },
            ]
        ));

        let sneak = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Sneak"),
            "Causes the player to sneak or move downward.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::GamepadButton(GamepadButton::East)
                },
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::Key(Key::LShift)
                },
            ]
        ));

        let toggle_pointer_lock = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Toggle pointer lock"),
            "Toggles whether the mouse should be locked to the center of the screen.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::Key(Key::T)
                },
            ]
        ));
        
        UserActions {
            look_horizontal,
            look_vertical,
            jump,
            move_forward,
            move_sideways,
            sneak,
            toggle_pointer_lock
        }
    }

    /// Updates the direction that the player is looking based the user input deltas.
    fn update_player_look_direction(transform: &mut Transform, pointer: Vec2, analog: Vec2) {
        /// The max speed at which rotation may occur with an analog input.
        const MAX_ROTATION_SPEED_ANALOG: f32 = 2.0;
        /// The max speed at which rotation may occur with a pointer delta.
        const MAX_ROTATION_SPEED_POINTER: f32 = 0.0025;
        let rot = transform.rotation;
        let (ux, uy, _) = rot.to_euler(EulerRot::YXZ);

        let ux = MAX_ROTATION_SPEED_ANALOG.mul_add(analog.x, MAX_ROTATION_SPEED_POINTER.mul_add(pointer.x, ux));
        let uy = MAX_ROTATION_SPEED_ANALOG.mul_add(-analog.y, MAX_ROTATION_SPEED_POINTER.mul_add(pointer.y, uy)).min(std::f32::consts::FRAC_PI_2 * 0.9).max(std::f32::consts::FRAC_PI_2 * -0.9);

        transform.rotation = Quat::from_euler(EulerRot::YXZ, ux, uy, 0.0);
    }

    /// Updates the position of the player based upon user input.
    fn update_player_position(transform: &mut Transform, delta_time: f32, motion: Vec3A) {
        /// The max speed that the player may move.
        const MAX_MOVEMENT_SPEED: f32 = 3.24 * 20.0 * 1.0;

        let mut walk_front = transform.look_direction();
        walk_front.y = 0.0;
        walk_front = walk_front.normalize_or_zero();
        let walk_right = Vec3A::Y.cross(walk_front);

        let normalized_horizontal = motion.xz() / motion.xz().length().max(1.0);
        let movement_power = walk_front.mul_add(Vec3A::splat(normalized_horizontal.y),
        walk_right.mul_add(Vec3A::splat(normalized_horizontal.x), vec3a(0.0, motion.y / motion.y.max(1.0), 0.0)));
        transform.position += (MAX_MOVEMENT_SPEED * delta_time * movement_power).into();
    }
}

impl WingsSystem for PlayerController {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<dyn FrameTiming>()
        .with::<dyn Input>()
        .with::<dyn Player>()
        .with::<dyn Raycaster>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::move_player);

    fn new(mut ctx: WingsContextHandle<Self>) -> Self {
        let user_actions = Self::get_user_actions(&mut ctx);
        Self {
            ctx,
            user_actions
        }
    }
}

/// Holds the set of actions relevant to user input.
#[derive(Copy, Clone, Debug)]
struct UserActions {
    /// Causes the player to look left or right.
    pub look_horizontal: ActionId<Analog>,
    /// Causes the player to look up or down.
    pub look_vertical: ActionId<Analog>,
    /// Causes the player to move upward.
    pub jump: ActionId<Digital>,
    /// Causes the player to walk forward or backward.
    pub move_forward: ActionId<Analog>,
    /// Causes the player to walk left or right.
    pub move_sideways: ActionId<Analog>,
    /// Causes the player to move downward.
    pub sneak: ActionId<Digital>,
    /// Toggles whether the mouse should be locked to the center of the screen.
    pub toggle_pointer_lock: ActionId<Digital>
}