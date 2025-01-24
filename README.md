<div align="center">
<img src="crowbar.png" alt="Crowbar Logo" width="128" height="128">
</div>
<h1 align="center">Crowbar - The <i>Fastest</i> Launcher<br />
<div align="center">
</div>
</h1>

Linux application launcher. Much fast. Much in alpha, so things might break.

## Installation & Setup

These instructions are written for Ubuntu, but should apply to most Linux
distributions.

1. Download the latest Crowbar binary from the [releases page](https://github.com/mxschll/crowbar/releases)

2. Install the binary:
   ```bash
   # Create the bin directory if it doesn't exist
   mkdir -p ~/.local/bin
   
   # Move the binary to your local bin directory
   mv crowbar ~/.local/bin/
   
   # Make it executable
   chmod +x ~/.local/bin/crowbar
   ```

3. Set up the keyboard shortcut in Ubuntu:
   1. Open System Settings
   2. Go to `Keyboard` settings
   3. Click on `View and Customize Shortcuts`
   4. Select `Custom Shortcuts`
   5. Click the `+` button to add a new shortcut
   6. Fill in the following:
      - Name: `Crowbar`
      - Command: `/home/YOUR_USERNAME/.local/bin/crowbar` (replace YOUR_USERNAME with your actual username)
      - Shortcut: Press your desired key combination (e.g., Super + Space)

Now you can launch Crowbar anytime by pressing your chosen keyboard shortcut!

> Note: Make sure to use the absolute path in the command field. For example, if your username is "john", 
> the command should be `/home/john/.local/bin/crowbar`

