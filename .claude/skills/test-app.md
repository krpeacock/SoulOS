# Test App Skill

Comprehensive skill for running SoulOS, taking screenshots, and testing app interactions.

## Usage

- `/test-app` - Runs the SoulOS app in the desktop simulator
- `/test-app screenshot [filename]` - Takes a screenshot of the running app
- `/test-app click <x> <y>` - Simulates a stylus tap at coordinates (x, y)
- `/test-app drag <x1> <y1> <x2> <y2>` - Simulates dragging from (x1,y1) to (x2,y2)
- `/test-app type <text>` - Enters text input
- `/test-app key <keyname>` - Presses a key (Home, Menu, AppA-D, PageUp, PageDown, etc.)
- `/test-app stop` - Stops the running app

## App Structure

SoulOS runs as a desktop simulator using SDL2 with these key features:

### Screen Dimensions
- Virtual screen: 240×320 portrait (3:4 ratio)
- Scaled 2x for visibility in simulator window (480×640)

### Navigation
- **System Strip**: Bottom area with Home | [App Name] | Menu sections
- **Home Button**: F5 or Home key returns to launcher
- **App Buttons**: F1-F4 map to AppA-D (quick launch first 4 apps)
- **Menu**: F6 or clicking Menu section in system strip

### Built-in Apps
1. **Notes** (AppA/F1) - Text notes with database persistence
2. **Address** (AppB/F2) - Contact management with vCard support  
3. **Date** - Calendar/date app
4. **ToDo** - Task management
5. **Mail** - Email client
6. **Calc** - Calculator
7. **Prefs** - Preferences/settings
8. **Draw** (AppH) - Drawing app with proportional scaling
9. **Sync** - Synchronization

### Input Methods
- **Mouse**: Left click = stylus tap, drag = stylus drag
- **Keyboard**: Full QWERTY with shift/caps support
- **Hard Buttons**: Escape=Power, F1-F6 for app functions, PageUp/Down

## Testing Workflows

### Basic Navigation Test
```bash
# Start the app
/test-app

# Navigate to Notes
/test-app key AppA
# or click on Notes icon
/test-app click 96 64

# Take screenshot of Notes app
/test-app screenshot notes_app

# Return to launcher
/test-app key Home
```

### Text Input Test
```bash
# Open Notes app
/test-app key AppA

# Type some text
/test-app type "Hello SoulOS!"

# Navigate with arrow keys
/test-app key ArrowLeft
/test-app key Backspace
/test-app type "World"

# Take screenshot
/test-app screenshot text_input_test
```

### Touch Interaction Test  
```bash
# Start at launcher
/test-app

# Click on Address app icon (position calculated from launcher grid)
/test-app click 144 64

# Simulate scrolling by dragging
/test-app drag 120 160 120 100

# Click on system strip Menu area
/test-app click 200 340

# Return home via system strip
/test-app click 40 340
```

## Technical Details

### Running the App
- Uses `cargo run` from the SoulOS root directory
- Binary: `soul-runner` crate
- HAL: `soul-hal-hosted` provides SDL2 desktop simulation
- Display: `embedded-graphics-simulator` for rendering

### Screenshot Implementation
Screenshots are captured by:
1. Reading the SDL2 display buffer from `SimulatorDisplay<Gray8>`
2. Converting grayscale data to standard image format
3. Saving to specified filename or auto-generated timestamp name

### Coordinate System
- Origin (0,0) at top-left
- App area: 240×320 (minus system strip at bottom)
- System strip: 20px high at bottom (y=320-340)
- Launcher icons: 32×32px in 4-column grid with spacing

### Key Mappings
- `Escape` → Power
- `F1-F4` → AppA-D (quick launch)
- `F5/Home` → Home button  
- `F6` → Menu button
- `PageUp/PageDown` → Page navigation
- Letters/numbers/symbols → Text input with shift/caps support

## Example Test Scripts

### Full App Tour
```bash
/test-app
/test-app screenshot 01_launcher
/test-app click 48 64    # Notes
/test-app screenshot 02_notes  
/test-app key Home
/test-app click 96 64    # Address
/test-app screenshot 03_address
/test-app key Home  
/test-app click 192 64   # Draw
/test-app screenshot 04_draw
/test-app stop
```

### Text Entry Workflow
```bash
/test-app
/test-app key AppA      # Open Notes
/test-app type "Meeting Notes"
/test-app key Enter
/test-app type "- Discuss project timeline"
/test-app key Enter  
/test-app type "- Review budget"
/test-app screenshot notes_with_content
/test-app key Home
/test-app stop
```