use std::time::*;
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
    /// Holds data about an object currently being dragged.
    dragged_object: Option<DraggedObject>,
    /// The kind of physics object to spawn next.
    object_kind: u32,
    /// The item that the user has currently selected.
    selected_item: u32,
    /// Holds handles for accessing user input.
    user_actions: UserActions,
    /// The time at which the user may next place or destroy voxels.
    wait_for_placement_until: Duration,
    /// Whether the user was placing or deleting voxels.
    was_placing: bool
}

impl PlayerController {
    /// Draws a crosshair and text related to the character controls.
    fn draw_controls_gui(&mut self, pointer_locked: bool) {
        let egui_system = self.ctx.get::<dyn egui::Egui>();
        let egui_ctx = egui_system.context();

        let mut painter = egui_ctx.layer_painter(egui::LayerId::background());
        if pointer_locked {
            Self::draw_crosshairs(&mut painter);
        }

        let selected_item = match self.selected_item % 8 {
            0 => "Red light",
            1 => "Green light",
            2 => "Blue light",
            3 => "White light",
            4 => "Red plastic",
            5 => "Green plastic",
            6 => "Blue plastic",
            7 => "White plastic",
            _ => unreachable!()
        };

        Self::draw_item_text(&mut painter, selected_item);
    }

    /// Handles interaction and dragging with physics objects.
    fn handle_object_interaction(&mut self, pointer_ray: &Ray, hit_result: Option<&RaycastHit>) {
        let input = self.ctx.get::<dyn Input>();
        let player = self.ctx.get::<dyn Player>();
        let drag_physics_entity = input.get(self.user_actions.drag_physics_entity);
        let spawn_physics_entity = input.get(self.user_actions.spawn_physics_entity);
        
        if spawn_physics_entity.pressed {
            let distance = hit_result.map(|x| x.distance).unwrap_or(f32::MAX);
            let position = pointer_ray.position + ((distance - 30.0).max(0.0).min(50.0) * pointer_ray.direction).into();
            self.object_kind = self.object_kind.wrapping_add(1);
            player.spawn_physics_object(position, self.object_kind);
        }

        if !drag_physics_entity.held && self.dragged_object.is_some() {
            player.drag_physics_object(None);
            self.dragged_object = None;
        }
        else if drag_physics_entity.pressed && self.dragged_object.is_none() {
            if let Some(hit) = hit_result {
                if let RaycastObject::Entity { id } = hit.object {
                    let contact_point = hit.voxel.as_vec3a() + Vec3A::splat(0.5);
                    self.dragged_object = Some(DraggedObject { contact_point, id, distance: hit.distance });
                }
            }
        }

        if let Some(dragged) = self.dragged_object {
            let target_position = pointer_ray.position + (dragged.distance * pointer_ray.direction).into();
            player.drag_physics_object(Some(DragEntity { contact_point: dragged.contact_point, id: dragged.id, target_position }));
        }
    }

    /// Places or destroys voxels according to the player's input.
    fn handle_player_place_destroy(&mut self, hit_result: Option<&RaycastHit>) {
        let input = self.ctx.get::<dyn Input>();
        let delete_voxels = input.get(self.user_actions.delete_voxels);
        let place_voxels = input.get(self.user_actions.place_voxels);
        let now = self.ctx.get::<dyn FrameTiming>().last_frame();        

        if let Some(hit) = hit_result {
            if hit.object == (RaycastObject::World { }) && self.wait_for_placement_until <= now { //self.dragged_object.is_none() && 
                let delete = if delete_voxels.pressed {
                    self.wait_for_placement_until = now + Duration::from_secs_f32(0.25);
                    self.was_placing = true;
                    true
                }
                else if self.was_placing && delete_voxels.held {
                    self.wait_for_placement_until = now + Duration::from_secs_f32(0.05);
                    true
                }
                else {
                    false
                };
    
                if delete {
                    self.ctx.get::<dyn Player>().delete_voxels_at(hit.voxel);
                }
                else {
                    let place = if place_voxels.pressed {
                        self.wait_for_placement_until = now + Duration::from_secs_f32(0.25);
                        self.was_placing = true;
                        true
                    }
                    else if self.was_placing && place_voxels.held {
                        self.wait_for_placement_until = now + Duration::from_secs_f32(0.05);
                        true
                    }
                    else {
                        false
                    };
    
                    if place {
                        self.ctx.get::<dyn Player>().place_voxels_at(hit.voxel, self.selected_item);
                    }
                }
            }
        }

        self.was_placing &= delete_voxels.held || place_voxels.held;
    }

    /// Updates the player's position based upon user input.
    /// Returns the player's new transform.
    fn move_player(&mut self) -> Transform {
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

        let lock_pointer = (input.pointer_locked() ^ toggle_pointer_lock.pressed) || (0.0 < look_vertical.abs().max(look_horizontal.abs()));
        input.set_pointer_locked(lock_pointer);
        
        drop(input);

        let mut player = self.ctx.get_mut::<dyn Player>();
        let mut transform = player.get_transform();

        Self::update_player_look_direction(&mut transform, pointer_delta, delta_time * vec2(look_horizontal, look_vertical));

        let net_vertical_motion = [0.0, 1.0][jump.held as usize] + [0.0, -1.0][sneak.held as usize];
        Self::update_player_position(&mut transform, delta_time, vec3a(move_sideways, net_vertical_motion, move_forward));
        
        player.set_transform(transform);
        drop(player);
        
        transform
    }

    /// Updates the item that the user currently has selected.
    fn update_selected_item(&mut self) {
        let input = self.ctx.get::<dyn Input>();
        let scroll_delta = input.scroll_delta();
        let toggle_item_left = input.get(self.user_actions.toggle_item_left);
        let toggle_item_right = input.get(self.user_actions.toggle_item_right);

        let net_toggle_item = scroll_delta.y + [0, -1][toggle_item_left.pressed as usize] + [0, 1][toggle_item_right.pressed as usize];
        self.selected_item = self.selected_item.wrapping_sub(net_toggle_item as u32);
    }

    /// Moves the player according to user inputs.
    fn handle_player_input(&mut self, _: &voxel_engine::timing::on::Frame) {
        /// The maximum distance away that the user may select something.
        const MAX_PLACEMENT_DISTANCE: f32 = 256.0;

        self.update_selected_item();
        let maybe_pointer_direction = self.ctx.get::<dyn Input>().pointer_direction();
        let transform = self.move_player();
        
        if let Some(direction) = maybe_pointer_direction {
            let pointer_ray = Ray {
                position: transform.position,
                direction,
                max_distance: MAX_PLACEMENT_DISTANCE
            };

            let hit_result = self.ctx.get::<dyn Raycaster>().cast(&pointer_ray);
            self.handle_object_interaction(&pointer_ray, hit_result.as_ref());
            self.handle_player_place_destroy(hit_result.as_ref());
        }

        let pointer_locked = self.ctx.get::<dyn Input>().pointer_locked();
        self.draw_controls_gui(pointer_locked);
    }

    /// Draws crosshairs on the center of the screen to help the player aim.
    fn draw_crosshairs(painter: &mut egui::Painter) {
        let center = painter.clip_rect().center();
        painter.rect_filled(egui::Rect::from_center_size(center, egui::vec2(14.0, 7.0)), 2.0, egui::Color32::BLACK);
        painter.rect_filled(egui::Rect::from_center_size(center, egui::vec2(7.0, 14.0)), 2.0, egui::Color32::BLACK);
        painter.rect_filled(egui::Rect::from_center_size(center, egui::vec2(10.0, 3.0)), 2.0, egui::Color32::WHITE);
        painter.rect_filled(egui::Rect::from_center_size(center, egui::vec2(3.0, 10.0)), 2.0, egui::Color32::WHITE);
    }

    /// Draws the provided text as a tooltip near the screen bottom.
    fn draw_item_text(painter: &mut egui::Painter, text: &str) {
        let center = painter.clip_rect().center();
        let bottom = painter.clip_rect().bottom();
        let text_position = egui::pos2(center.x, bottom - 50.0);

        let rect_shape = painter.add(egui::Shape::Noop);
        let text_rect = painter.text(text_position, egui::Align2::CENTER_CENTER, text, egui::FontId::default(), egui::Color32::WHITE);
        painter.set(rect_shape, egui::Shape::rect_filled(text_rect.expand(5.0), 3.0, egui::Color32::from_black_alpha(64)));
    }

    /// Registers the set of actions relevant to player movement.
    fn get_user_actions(ctx: &mut WingsContextHandle<Self>) -> UserActions {
        let mut input = ctx.get_mut::<dyn Input>();

        let delete_voxels = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Delete"),
            "Deletes voxels where the player's pointer is.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::GamepadButton(GamepadButton::LeftTrigger)
                },
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::MouseButton(MouseButton::Left)
                },
            ]
        ));

        let drag_physics_entity = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Drag entity"),
            "Drags a physics entity around the scene.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::GamepadButton(GamepadButton::RightTrigger)
                },
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::MouseButton(MouseButton::Right)
                },
            ]
        ));

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

        let place_voxels = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Place"),
            "Places voxels where the player's pointer is.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::GamepadButton(GamepadButton::RightTrigger)
                },
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::MouseButton(MouseButton::Right)
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

        let spawn_physics_entity = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Spawn debug entity"),
            "Spawns a physics entity for debugging.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::GamepadButton(GamepadButton::North)
                },
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::Key(Key::F)
                },
            ]
        ));

        let toggle_item_left = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Toggle item (left)"),
            "Toggles the selected item to the left.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::GamepadButton(GamepadButton::DPadLeft)
                },
            ]
        ));

        let toggle_item_right = input.define(ActionDescriptor::new(
            ActionName::new::<Self>("Toggle item (right)"),
            "Toggles the selected item to the right.",
            &[
                DigitalBinding {
                    threshold: 0.9,
                    raw_input: RawInput::GamepadButton(GamepadButton::DPadRight)
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
                }
            ]
        ));
        
        UserActions {
            drag_physics_entity,
            delete_voxels,
            look_horizontal,
            look_vertical,
            jump,
            move_forward,
            move_sideways,
            place_voxels,
            sneak,
            spawn_physics_entity,
            toggle_item_left,
            toggle_item_right,
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
        .with::<dyn egui::Egui>()
        .with::<dyn FrameTiming>()
        .with::<dyn Input>()
        .with::<dyn Player>()
        .with::<dyn Raycaster>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::handle_player_input);

    fn new(mut ctx: WingsContextHandle<Self>) -> Self {
        let dragged_object = None;
        let object_kind = 0;
        let selected_item = 0;
        let wait_for_placement_until = Duration::ZERO;
        let was_placing = false;
        let user_actions = Self::get_user_actions(&mut ctx);

        Self {
            ctx,
            dragged_object,
            object_kind,
            selected_item,
            user_actions,
            wait_for_placement_until,
            was_placing
        }
    }
}

/// Stores information about an object being dragged.
#[derive(Copy, Clone, Debug)]
struct DraggedObject {
    /// The body-local point at which the object was grabbed.
    pub contact_point: Vec3A,
    /// The ID of the entity being dragged.
    pub id: u64,
    /// The distance that the entity is held away from the player.
    pub distance: f32
}

/// Holds the set of actions relevant to user input.
#[derive(Copy, Clone, Debug)]
struct UserActions {
    /// Deletes voxels where the player's pointer is.
    pub delete_voxels: ActionId<Digital>,
    /// Drags a physics entity around the screen.
    pub drag_physics_entity: ActionId<Digital>,
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
    /// Places voxels where the player's pointer is.
    pub place_voxels: ActionId<Digital>,
    /// Causes the player to move downward.
    pub sneak: ActionId<Digital>,
    /// Spawns a physics entity for debugging.
    pub spawn_physics_entity: ActionId<Digital>,
    /// Toggles the selected item to the left.
    pub toggle_item_left: ActionId<Digital>,
    /// Toggles the selected item to the right.
    pub toggle_item_right: ActionId<Digital>,
    /// Toggles whether the mouse should be locked to the center of the screen.
    pub toggle_pointer_lock: ActionId<Digital>
}