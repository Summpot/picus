use bevy_ecs::entity::Entity;
use masonry::{
    accesskit::{Node, Role},
    core::{
        AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, PaintCtx, PropertiesRef,
        QueryCtx, RegisterCtx, UpdateCtx, Widget, WidgetMut, WidgetPod, WidgetRef,
    },
    kurbo::{Axis, Point, Size},
    layout::{LayoutSize, LenReq},
};
use vello::Scene;

/// Thin wrapper widget that binds one synthesized ECS entity to one Masonry widget id.
pub struct EntityScopeWidget {
    entity: Entity,
    child: WidgetPod<dyn Widget>,
}

impl EntityScopeWidget {
    #[must_use]
    pub fn new(entity: Entity, child: NewWidget<impl Widget + ?Sized>) -> Self {
        Self {
            entity,
            child: child.erased().to_pod(),
        }
    }

    pub fn set_entity(this: &mut WidgetMut<'_, Self>, entity: Entity) {
        this.widget.entity = entity;
    }

    pub fn child_mut<'t>(this: &'t mut WidgetMut<'_, Self>) -> WidgetMut<'t, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }
}

impl Widget for EntityScopeWidget {
    type Action = ();

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn property_changed(&mut self, _ctx: &mut UpdateCtx<'_>, _property_type: std::any::TypeId) {}

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<f64>,
    ) -> f64 {
        let auto_length = len_req.into();
        let context_size = LayoutSize::maybe(axis.cross(), cross_length);
        ctx.compute_length(
            &mut self.child,
            auto_length,
            context_size,
            axis,
            cross_length,
        )
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.run_layout(&mut self.child, size);
        ctx.place_child(&mut self.child, Point::ORIGIN);
        ctx.derive_baselines(&self.child);
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {}

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.child.id()])
    }

    fn find_widget_under_pointer<'c>(
        &'c self,
        ctx: QueryCtx<'c>,
        pos: Point,
    ) -> Option<WidgetRef<'c, dyn Widget>> {
        if ctx.is_stashed() {
            return None;
        }

        let local_pos = ctx.window_transform().inverse() * pos;

        if let Some(clip) = ctx.clip_path()
            && !clip.contains(local_pos)
        {
            return None;
        }

        for child_id in self.children_ids().iter().rev() {
            let child_ref = ctx.get(*child_id);
            if let Some(child) = child_ref.find_widget_under_pointer(pos) {
                return Some(child);
            }
        }

        None
    }

    fn get_debug_text(&self) -> Option<String> {
        Some(format!("entity_scope={}", self.entity.to_bits()))
    }
}
