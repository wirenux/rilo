- [x] Install Crossterm
- [x] Enter raw mode
- [x] Add the read part of the editor
- [x] Add the write part of the editor
- [x] Add the search part of the editor
- [x] Add the higlighting part of the editor
- [x] Rust higlighting
- [x] C higlighting
- [x] Python higlighting
- [x] Add better welcome message with color, github link etc...
    - [x] Say that ESC in a prompt exit the prompt
- [x] change key to use on macos (like no page up etc... and use `Ctrl+...` which is better ig)
- [x] able to `save as` even for existing file with `ctrl-shift-s`
    - [x] put it in the welcome message
- [x] when searching search on the whole line not just the start
- [?] add `ctrl-z` and `ctrl-shift-z` to undo and redo
    - [?] add something like a stack
    - [?] put it in the welcome message
- [x] add config file like `~/.rilorc`
    - [x] open the file
    - [x] read for `TAB_STOP=X`
    - [x] read for `QUIT_TIMES=X`
    - [x] read this file at each launch
    - [x] Do nothing if file not present (or maybe add a little status message)
    - [x] talk about it in welcome message
- [x] add hex in c support
- [x] add `#include` and `#define` c support
- [x] add line number on the 1st colon only when there is text
    - [x] Get total line of file to know how much number
    - [x] able to turn off with `~/.rilorc`
- [x] auto-indent
    - [x] search in `~/.rilorc`

- [x] make the editor about 1000 lines of code

- [x] Do a `// TODO` check

- [x] **(LAST)** change version to v1.0.0
- [x] **(LAST)** create a github release
- [x] **(LAST)** do the thing to choose file in the crates.io project
- [x] **(LAST)** upload on `crates.io`

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
- `~/.rilorc`
- auto-indent

choice :

- no incremental search
- hex support in c

workaround:

- not all term support ctrl+shift+s so ctrl+o does the same thing (save as...)

control : (do a table)

do a side by side comp with vim (to see the same color)

debug:

- to get the log / print in  the term : open to term. In the 2nd one type `tty` (to get the tty number) and in the main one type : `cargo run 2>/dev/TTY_NUMBER_YOU_GOT`

explain how `~/.rilorc` work and what are the settings that you can put

# status

Current step : Guide finished ! :D
