# VHS demo tape for pui - Pueue TUI
# Uses https://github.com/charmbracelet/vhs
# Run: vhs demo.tape

Output demo.gif

Require pui
Require pueue

Set Shell zsh
Set FontSize 16
Set Width 1000
Set Height 500
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set Margin 20
Set MarginFill "#585b70" #Catppuccin Mocha's "Surface 2" color, a muted lavender-gray
Set BorderRadius 10
Set TypingSpeed 50ms
Set Padding 0

# ── Setup: reset pueue and add dummy tasks ──────────────────────────

Hide
Type "pueue reset -f && sleep 0.5"
Enter
Type "pueue add -- sleep 300"
Enter
Type "pueue add -- 'echo hello world'"
Enter
Type "pueue add -s -- 'echo task finished successfully'"
Enter
Type "pueue add -- false"
Enter
Type `pueue add -s 'for i in $(seq 1 20); do echo "Processing item $i..."; sleep 1; done'`
Enter
Type "pueue add -s ls -la"
Enter
Type "clear"
Enter
Show

# ── Title card: launch & navigate ────────────────────────────────────

Type@0 "# Launch pui and navigate"
Sleep 1500ms
Ctrl+C
Type "pui"
Sleep 600ms
Enter
Sleep 2s

Down
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 600ms
Down
Sleep 600ms
Up
Sleep 600ms
Up
Sleep 1s

# ── Title card: start task ───────────────────────────────────────────

Hide
Type "q"
Type "clear"
Enter
Show
Type@0 "# Press r to start a task"
Sleep 1500ms
Hide
Ctrl+C
Type "pui"
Enter
Down
Down
Down
Down
Show
Sleep 500ms

Type "r"
Sleep 2s

# ── Title card: view logs ────────────────────────────────────────────

Hide
Type "q"
Type "clear"
Enter
Show
Type@0 "# Press Enter to view streaming logs"
Sleep 1500ms
Hide
Ctrl+C
Type "pui"
Enter
Down
Down
Down
Down
Enter
Show
Sleep 3s

# ── Title card: filter ───────────────────────────────────────────────

Hide
Escape
Type "q"
Type "clear"
Enter
Show
Type@0 "# Press f to filter tasks"
Sleep 1500ms
Hide
Ctrl+C
Type "pui"
Enter
Sleep 500ms
Show

Sleep 500ms
Type "f"
Sleep 500ms
Type "echo"
Sleep 2s
Escape
Sleep 1s

# ── Title card: help ─────────────────────────────────────────────────

Hide
Type "q"
Type "clear"
Enter
Show
Type@0 "# Press ? for help"
Sleep 1500ms
Hide
Ctrl+C
Type "pui"
Enter
Show

Type "?"
Sleep 3s
Escape
Sleep 1s
