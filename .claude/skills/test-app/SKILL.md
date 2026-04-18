---
name: test-app
description: Workflow for creating, testing, and verifying a SoulOS application using the MobileBuilder framework and soul-db storage. Use when building new apps or adding features to existing ones.
---

# Test App Workflow

This skill guides you through the process of building a SoulOS application using the MobileBuilder paradigm, ensuring persistence, interactivity, and accessibility.

## 1. App Design (The MobileBuilder Pattern)
SoulOS apps are data-driven. Instead of hardcoding UI, use the `soul-ui::Form` schema.
- **Form Name**: Unique identifier for the app's UI.
- **Components**: Buttons, Labels, Inputs, and Checkboxes.
- **Persistence**: Store the JSON Form in a `soul_db` record.

## 2. Implementation Steps
1. **Define the Schema**: Create a JSON representation of your app's screens.
2. **Seed the Database**: Ensure the `.sdb` file contains the initial Form definition.
3. **Register the App**: Add the new app to `APPS` in `soul-runner/src/main.rs`.
4. **Implement Logic**: Use the `interactions` array in the JSON to wire up triggers (`OnTap`) to actions.

## 3. Testing & Verification
A feature is not done until verified.
- **Manual Verification**: Launch the app in the hosted runner.
- **Automation**: Use `test_soulos.py` or `test_automation.py` to simulate pen events and verify state.
- **State Inspection**: Inspect the `.sdb` records directly to ensure `SaveRecord` and `DeleteRecord` actions are working.

## 4. Current Limitations
- **Keyboard Editing**: Builder labels must currently be edited via the "Edit Label" menu which uses a modal `TextInput`. Directly tapping a label to type is not yet implemented.
- **Complex Menus**: The builder menu is a simple list and can become crowded; prioritize space-efficient layouts for multi-tool builders.
- **Primitive Availability**: If a primitive like "Checkbox" is missing, it must be added to `soul_ui::form::ComponentType` and `Form::draw` first.
