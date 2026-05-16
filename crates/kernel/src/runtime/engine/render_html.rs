use crate::runtime::types::{RuntimeSceneView, RuntimeState};

pub fn render_runtime_html(view: &RuntimeSceneView, state: &RuntimeState) -> String {
    format!(
        "<section><h3>{}</h3><p>phase: {}</p><p>countdown: {}</p><p>current_time: {:.1} {}</p><p>rate: {}</p><p>inventory: {}</p><p>timeline: {}</p></section>",
        view.scene_id,
        view.phase,
        view.countdown,
        view.current_time,
        view.time_unit,
        view.time_rate,
        state.inventory.join(", "),
        state.timeline.join(" | "),
    )
}
