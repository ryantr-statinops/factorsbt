"""Public exception types for SDK transport and contract failures."""


class FactorsBTError(Exception):
    """Base class for errors raised by the FactorsBT SDK."""


class TransportError(FactorsBTError):
    """The backend could not be reached or returned an unusable response."""


class RequestTimeout(TransportError):
    """The backend did not respond before the configured timeout."""


class HttpStatusError(FactorsBTError):
    """The backend returned an unsuccessful HTTP status."""

    def __init__(self, status: int):
        self.status = status
        super().__init__(f"backend returned HTTP {status}")


class ProtocolError(FactorsBTError):
    """The backend response did not match the health wire contract."""
