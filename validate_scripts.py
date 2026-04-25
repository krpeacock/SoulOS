#!/usr/bin/env python3
"""
SoulOS Rhai Script Validator

A simple script to validate all Rhai scripts in the assets/scripts directory
by running cargo and parsing the output for compilation errors.
"""

import subprocess
import sys
import os
import glob

def validate_rhai_scripts():
    """Validate all Rhai scripts by running cargo and checking for errors."""
    
    # Ensure we're in the right directory
    if not os.path.exists('Cargo.toml'):
        print("❌ Error: Not in SoulOS root directory (no Cargo.toml found)")
        return False
    
    # Find all Rhai scripts
    script_files = glob.glob('assets/scripts/*.rhai')
    if not script_files:
        print("⚠️  Warning: No Rhai scripts found in assets/scripts/")
        return True
    
    print(f"🔍 Found {len(script_files)} Rhai script(s):")
    for script in sorted(script_files):
        print(f"   - {script}")
    print()
    
    # Run cargo to test compilation
    print("🚀 Running cargo to test script compilation...")
    try:
        result = subprocess.run(
            ['cargo', 'run', '--quiet'],
            capture_output=True,
            text=True,
            timeout=30
        )
        
        # Check for compilation errors in stderr
        error_lines = []
        for line in result.stderr.split('\n'):
            if 'Failed to compile' in line:
                error_lines.append(line)
        
        if error_lines:
            print("❌ Compilation errors found:")
            for error in error_lines:
                print(f"   {error}")
            return False
        
        # Check for warnings
        warning_lines = []
        for line in result.stderr.split('\n'):
            if 'warning:' in line and 'unused' in line:
                warning_lines.append(line.strip())
        
        if warning_lines:
            print("⚠️  Warnings found:")
            for warning in warning_lines[:5]:  # Show max 5 warnings
                print(f"   {warning}")
            if len(warning_lines) > 5:
                print(f"   ... and {len(warning_lines) - 5} more warnings")
            print()
        
        print("✅ All Rhai scripts compiled successfully!")
        return True
        
    except subprocess.TimeoutExpired:
        print("❌ Timeout: Cargo took too long to run")
        return False
    except FileNotFoundError:
        print("❌ Error: 'cargo' command not found. Please install Rust.")
        return False
    except Exception as e:
        print(f"❌ Error running cargo: {e}")
        return False

def main():
    """Main entry point."""
    print("SoulOS Rhai Script Validator")
    print("=" * 40)
    
    success = validate_rhai_scripts()
    
    if success:
        print("\n🎉 Validation complete - all scripts are valid!")
        sys.exit(0)
    else:
        print("\n💥 Validation failed - please fix the errors above")
        sys.exit(1)

if __name__ == '__main__':
    main()