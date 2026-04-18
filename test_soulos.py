#!/usr/bin/env python3
"""
SoulOS Claude Testing Interface
Provides Claude with Playwright-like testing capabilities for SoulOS
"""

import subprocess
import time
import os
from typing import Tuple, Optional, List, Dict, Any
import json

class SoulOSTestInterface:
    """
    Testing interface that allows Claude to interact with SoulOS programmatically.
    This provides Playwright-like capabilities for automated testing.
    """
    
    def __init__(self, window_name: str = "SoulOS"):
        self.window_name = window_name
        self.screenshot_dir = "/tmp/soulos_tests"
        self.screenshot_counter = 0
        
        # Create screenshot directory if it doesn't exist
        os.makedirs(self.screenshot_dir, exist_ok=True)
        
    def screenshot(self, name: Optional[str] = None) -> str:
        """
        Take a screenshot of the current state
        Returns the path to the screenshot file
        """
        if name is None:
            name = f"step_{self.screenshot_counter:03d}"
            self.screenshot_counter += 1
        
        filepath = os.path.join(self.screenshot_dir, f"{name}.png")
        result = subprocess.run(["screencapture", "-x", filepath], capture_output=True)
        
        if result.returncode != 0:
            raise Exception(f"Screenshot failed: {result.stderr.decode()}")
        
        return filepath
    
    def click(self, x: int, y: int, wait_ms: int = 500) -> str:
        """
        Click at the specified coordinates
        Returns path to screenshot taken after click
        """
        result = subprocess.run(["cliclick", f"c:{x},{y}"], capture_output=True)
        if result.returncode != 0:
            raise Exception(f"Click failed: {result.stderr.decode()}")
        
        if wait_ms > 0:
            time.sleep(wait_ms / 1000.0)
        
        return self.screenshot(f"after_click_{x}_{y}")
    
    def type_text(self, text: str, wait_ms: int = 100) -> str:
        """
        Type text character by character
        Returns path to screenshot taken after typing
        """
        for char in text:
            if char == ' ':
                subprocess.run(["cliclick", "kp:space"], capture_output=True)
            elif char == '\n':
                subprocess.run(["cliclick", "kp:return"], capture_output=True)
            else:
                # Escape special characters
                escaped_char = char.replace("'", "\\'").replace('"', '\\"')
                subprocess.run(["cliclick", f"t:{escaped_char}"], capture_output=True)
            time.sleep(wait_ms / 1000.0)
        
        return self.screenshot("after_typing")
    
    def press_key(self, key: str, wait_ms: int = 500) -> str:
        """
        Press a special key (return, tab, escape, etc.)
        Returns path to screenshot taken after key press
        """
        result = subprocess.run(["cliclick", f"kp:{key}"], capture_output=True)
        if result.returncode != 0:
            raise Exception(f"Key press failed: {result.stderr.decode()}")
        
        if wait_ms > 0:
            time.sleep(wait_ms / 1000.0)
        
        return self.screenshot(f"after_key_{key}")
    
    def wait(self, seconds: float) -> None:
        """Wait for the specified number of seconds"""
        time.sleep(seconds)
    
    # SoulOS-specific methods
    def launch_notes_app(self) -> str:
        """Launch the Notes app from the launcher"""
        return self.click(363, 170, wait_ms=800)  # Notes app coordinates
    
    def launch_address_app(self) -> str:
        """Launch the Address app from the launcher"""
        return self.click(320, 170, wait_ms=800)  # Address app coordinates
    
    def launch_draw_app(self) -> str:
        """Launch the Draw app from the launcher"""
        return self.click(318, 225, wait_ms=800)  # Draw app coordinates
    
    def press_home_button(self) -> str:
        """Press the Home button to return to launcher"""
        # F5 is mapped to Home in SoulOS
        return self.press_key("f5", wait_ms=800)
    
    def press_menu_button(self) -> str:
        """Press the Menu button"""
        # F6 is mapped to Menu in SoulOS
        return self.press_key("f6", wait_ms=500)
    
    def test_notes_app_workflow(self) -> Dict[str, Any]:
        """
        Complete workflow test for the Notes app
        Returns a test result dictionary
        """
        test_result = {
            "test_name": "Notes App Workflow",
            "steps": [],
            "screenshots": [],
            "success": True,
            "error": None
        }
        
        try:
            # Step 1: Initial state
            initial = self.screenshot("initial_state")
            test_result["steps"].append("Captured initial state")
            test_result["screenshots"].append(initial)
            
            # Step 2: Launch Notes app
            after_launch = self.launch_notes_app()
            test_result["steps"].append("Launched Notes app")
            test_result["screenshots"].append(after_launch)
            
            # Step 3: Type some content
            test_content = "This is a test note created by Claude!"
            after_typing = self.type_text(test_content)
            test_result["steps"].append(f"Typed content: '{test_content}'")
            test_result["screenshots"].append(after_typing)
            
            # Step 4: Press Enter to confirm/save
            after_enter = self.press_key("return")
            test_result["steps"].append("Pressed Enter")
            test_result["screenshots"].append(after_enter)
            
            # Step 5: Return to home
            after_home = self.press_home_button()
            test_result["steps"].append("Returned to home")
            test_result["screenshots"].append(after_home)
            
            test_result["steps"].append("Test completed successfully")
            
        except Exception as e:
            test_result["success"] = False
            test_result["error"] = str(e)
            test_result["steps"].append(f"Test failed with error: {e}")
        
        return test_result
    
    def print_test_result(self, result: Dict[str, Any]) -> None:
        """Print a formatted test result"""
        print(f"\n{'='*50}")
        print(f"Test: {result['test_name']}")
        print(f"Result: {'✅ PASSED' if result['success'] else '❌ FAILED'}")
        print(f"{'='*50}")
        
        print("\nSteps executed:")
        for i, step in enumerate(result['steps'], 1):
            print(f"  {i}. {step}")
        
        print(f"\nScreenshots captured: {len(result['screenshots'])}")
        for screenshot in result['screenshots']:
            print(f"  📷 {screenshot}")
        
        if result['error']:
            print(f"\n❌ Error: {result['error']}")
        
        print()

def main():
    """Demo/test the SoulOS testing interface"""
    print("SoulOS Testing Interface - Claude's Playwright-like Tool")
    print("=" * 60)
    
    tester = SoulOSTestInterface()
    
    # Run the Notes app workflow test
    notes_result = tester.test_notes_app_workflow()
    tester.print_test_result(notes_result)

if __name__ == "__main__":
    main()