import curses


# Terminal handling in a separate thread
def terminal_thread(stdscr):
    curses.curs_set(0)  # Hide cursor
    stdscr.clear()  # Clear the screen
    stdscr.addstr("Initializing HPC@Cloud CLI...\n")
    stdscr.refresh()

    # Here you can add more terminal UI logic as needed
    # For now, let's just wait for a keypress to exit
    stdscr.getch()
