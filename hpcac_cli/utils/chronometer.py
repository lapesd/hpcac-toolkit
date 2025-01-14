import time


class Chronometer:
    def __init__(self):
        self.start_time = None  # Time when the chronometer started
        self.elapsed_time = 0  # Total elapsed time
        self.running = False  # Tracks whether the chronometer is running

    def start(self):
        """Start the chronometer."""
        if not self.running:
            self.start_time = time.time()
            self.running = True
        else:
            raise RuntimeError("Chronometer is already running!")

    def stop(self):
        """Stop the chronometer and add the elapsed time since the last start."""
        if self.running:
            if self.start_time is not None:
                self.elapsed_time += time.time() - self.start_time
                self.running = False
                self.start_time = None
            else:
                raise ValueError("Chronometer is running but start_time is None!")

    def resume(self):
        """Resume the chronometer without resetting elapsed time."""
        if not self.running:
            self.start_time = time.time()
            self.running = True

    def reset(self):
        """Reset the chronometer."""
        self.start_time = None
        self.elapsed_time = 0
        self.running = False

    def get_elapsed_time(self):
        """
        Get the total elapsed time in seconds.
        If the chronometer is running, include the time since the last start.
        """
        if self.running:
            if self.start_time is not None:
                return self.elapsed_time + (time.time() - self.start_time)
            else:
                raise ValueError("Chronometer is running but start_time is None!")
        return self.elapsed_time
