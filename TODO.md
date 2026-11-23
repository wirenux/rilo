- [x] Install Crossterm
- [x] Enter raw mode
- [x] Add the read part of the editor
- [x] Add the write part of the editor
- [x] Add the search part of the editor
- [x] Add the higlighting part of the editor
- [ ] Rust higlighting
- [x] C higlighting
- [ ] Python higlighting
- [x] Add better welcome message with color, github link etc...
    - [x] Say that ESC in a prompt exit the prompt
- [ ] change key to use on macos (like no page up etc... and use `Ctrl+...` which is better ig)
- [ ] able to `save as` even for existing file with `ctrl-shift-s`
    - [ ] put it in the welcome message
- [ ] when searching search on the whole line not just the start
- [ ] add `ctrl-z` and `ctrl-shift-z` to undo and redo
    - [ ] add something like a stack
    - [ ] put it in the welcome message
- [ ] add config file like `~/.rilorc`
    - [ ] open the file
    - [ ] read for `TAB_STOP=X`
    - [ ] read for `QUIT_TIMES=X`
    - [ ] read this file at each launch
    - [ ] Do nothing if file not present (or maybe add a little status message)
- [x] add hex in c support
- [x] add `#include` and `#define` c support
- [ ] add line number on the 1st colon only when there is text

- [ ] Do a `// TODO` check

- [ ] **(LAST)** change version to v1.0.0
- [ ] **(LAST)** create a github release
- [ ] **(LAST)** upload on `crates.io`

# say in readme :

func :

- status bar
- keybinds
- message bar with message gone after 3 sec
- maybe ? : del_char, new_line (even at the middle)
- color
- save as (for new file)
- color for `Error:`, `Warning:`, `Info:`, `Help:`
- same color as vim
- use `queue!()` for less/no flicker
- hex support in c
- `#define` and `#include` support in c

choice :

- no incremental search
- hex support in c

control : (do a table)

do a side by side comp with vim (to see the same color)

# status

Current step : Guide finished ! :D
