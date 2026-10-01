use crate::{
    ui::{
        component::widgets::focused_block,
        widget::input_manager::{InputManagerWidget, InputManagerWidgetState},
    },
    ui_lib::{
        command::{Direction2D, PointerEvent, PointerGesture},
        component::{Component, EventOutcome},
        events::MoveEvent,
        theme::DEFAULT_THEME,
    },
};

#[derive(Debug, Default)]
pub struct ShortCutView {
    id: uuid::Uuid,
    state: InputManagerWidgetState,
}

impl Component for ShortCutView {
    fn id(&self) -> crate::ui_lib::component::ComponentId {
        self.id
    }

    fn kind() -> crate::ui_lib::component::ComponentKind
    where
        Self: Sized,
    {
        "short cut view"
    }

    fn render(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
        ctx: crate::ui_lib::component::RenderContext<'_>,
    ) {
        let block = focused_block("Keyboard bindings", ctx.focused);
        self.state.set_focused(ctx.focused_kind);
        frame.render_stateful_widget(
            InputManagerWidget::new(ctx.manager, &DEFAULT_THEME).block(block),
            area,
            &mut self.state,
        );
    }

    fn on(
        &mut self,
        event: Box<dyn crate::event::Event>,
    ) -> crate::ui_lib::component::EventOutcome {
        if let Some(MoveEvent(event)) = event.downcast_ref::<MoveEvent>() {
            match event {
                Direction2D::Up => self.state.scroll_up(),
                Direction2D::Down => self.state.scroll_down(),
                Direction2D::Left | Direction2D::Right => {}
            }
        }
        if let Some(event) = event.downcast_ref::<PointerEvent>() {
            match event.gesture {
                PointerGesture::ScrollUp => self.state.scroll_up(),
                PointerGesture::ScrollDown => self.state.scroll_down(),
                _ => {}
            }
        }

        EventOutcome::ignored(event)
    }
}
