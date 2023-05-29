class ConfigurationError(Exception):
    """Raised when there's an error in the Cluster configuration."""

    def __init__(self, message):
        super().__init__(message)
