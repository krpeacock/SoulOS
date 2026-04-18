# MobileBuilder for SoulOS: Product Design Document (PDD)

## 1. Executive Summary & Vision
MobileBuilder is the sovereign workshop of SoulOS. Drawing inspiration from the GRiD Systems and PenRight! lineage (which culminated in PalmOS RAD environments), it transforms the handheld from a passive consumption device into a self-hosting production tool. It empowers users, power-users, and autonomous coding agents to construct, modify, and deploy application interfaces entirely on-device, adhering strictly to the **Zen of Palm**—ruthless simplification, immediate execution, and a paper-replacement metaphor.

## 2. Historical Context & Core Philosophy
Early mobile Rapid Application Development (RAD) environments like MobileBuilder and Squeak Smalltalk proved that resource-constrained devices could be powerful workspaces. They prioritized:
- **Visual Forms Over Code**: The interface *is* the application's skeletal structure.
- **Cross-Platform Tenacity**: Write once, run everywhere (a philosophy mirrored by SoulOS's strict `no_std` HAL boundary).
- **The 80/20 Rule**: Most mobile applications are "forms-over-data" (80%). The builder targets this specifically, abstracting away complex event loops and memory management so that the creation of lists, buttons, and text fields is instantaneous.

## 3. Product Functional Specification

### 3.1. The Visual Canvas (WYSIWYG Workspace)
- **Direct Manipulation**: The 240×320 screen acts as the canvas. Components can be tapped to select, dragged to reposition, and pulled via handles to resize.
- **In-Place Redesign**: Apps like `Draw` or `Notes` can be thrown into "Edit Layout" mode at any time. The live application state pauses, and the UI becomes a malleable canvas, fulfilling the requirement to "redesign the app in the app itself."

### 3.2. Component Primitives
The builder supports a core set of stateless and stateful primitives:
- `Button`: Trigger actions.
- `Label`: Static text display.
- `TextInput`: Single-line data entry.
- `TextArea`: Multi-line, scrollable data entry.
- `Canvas`: A drawing surface.

### 3.3. Element Management & Deletion
- **Selection**: A selected component displays 8-way resize handles and a dotted bounding box.
- **Deletion**: A selected element can be deleted via a hardware button (e.g., `PageDown` mapped to delete in edit mode) or via a contextual "Delete Element" menu option. This ensures the canvas remains clean and iterative prototyping is frictionless.

### 3.4. Property Inspector & Label Editing
To move beyond wireframes, components must be customized:
- **Textual Editing**: When a component is selected, a new menu option ("Edit Properties") invokes a `TextInput` modal overlay. This allows the user to change a `Button`'s label or a `Label`'s text instantly.
- **Agent Interoperability**: Because the state is purely JSON within a `soul_db` record, a coding agent can read the `.sdb` file, find the component by `id`, and inject complex textual properties without requiring GUI interaction.

### 3.5. Rudimentary Interaction Logic (The "Action" System)
An interface is useless without behavior. MobileBuilder introduces a zero-code "Tag-and-Trigger" logic system embedded directly into the component's JSON definition.
- **Triggers**: `OnTap`, `OnLongPress`, `OnChange`.
- **Actions**:
  - `Navigate(FormID)`: Replaces the current form with another, creating multi-screen applications.
  - `SaveRecord(Category)`: Serializes the current state of all input fields into the active `Database`.
  - `DeleteRecord()`: Removes the currently loaded record.
  - `CloseApp()`: Exits the application.

*Example Agent-Inspectable JSON Structure:*
```json
{
  "id": "btn_save",
  "type_": "Button",
  "bounds": {"x": 10, "y": 280, "w": 60, "h": 20},
  "properties": {"label": "Save Note"},
  "a11y": {"label": "Save the current note", "role": "button"},
  "actions": [
    {"trigger": "OnTap", "action": "SaveRecord", "params": {"category": 0}}
  ]
}
```

### 3.6. Accessibility (A11y) as a First-Class Citizen
Accessibility is not an afterthought; it is a structural mandate.
- Every `Component` schema mandates an `A11yHints` block containing a `label` and a `role`.
- The Builder UI visually flags (e.g., via a small warning icon) any component where the accessibility label is identical to the auto-generated ID, forcing the designer (or the agent) to provide meaningful context for screen readers.

## 4. Technical Architecture & Constraints
- **JSON Payload**: The entire UI form is serialized to JSON and stored in a `soul_db::Database` record (e.g., `draw_ui.sdb`). This format is human-readable, agent-inspectable, and avoids opaque binary blobs.
- **`no_std` Compliance**: The builder and its rendering engine reside in `soul-ui`, relying solely on `alloc` and `embedded-graphics`, ensuring it compiles natively for e-ink hardware.
- **Event Delegation**: The `EditOverlay` acts as an event interceptor. When inactive, `Event::PenDown` passes to the app. When active, it intercepts the event to perform hit-testing on the resize handles or bounding boxes.

## 5. Implementation Roadmap (Iterative Specification)

### Phase 1: Foundation (Completed)
- [x] JSON Schema (`Form`, `Component`, `Rect`, `A11yHints`).
- [x] Storage backend (`soul_db` integration).
- [x] Basic `EditOverlay` with hit-testing and dragging.
- [x] Integration into `Draw` and a standalone `MobileBuilder` app.

### Phase 2: Component Customization & Deletion (Pending)
- [ ] Implement component deletion (e.g., pressing `PageDown` while an element is selected, or a menu item).
- [ ] Implement Label/Property editing: A modal that allows typing a new string into `properties["label"]`.

### Phase 3: Rudimentary Logic & Navigation (Pending)
- [ ] Expand the JSON schema to include an `Action` enum and an `actions` array on `Component`.
- [ ] Build an Action Dispatcher in `Form::hit_test` that returns triggered actions back to the hosting `App` (so the App knows to switch screens or save data).
- [ ] Add the ability to create secondary Forms/Screens and wire a Button to `Navigate` to them.

## 6. Conclusion
MobileBuilder elevates SoulOS from a static OS to a dynamic, self-modifiable environment. By marrying the simplicity of PalmOS RAD tools with modern JSON schemas, it creates a robust ecosystem where users construct their tools, and AI agents can seamlessly inspect, debug, and augment those same creations.