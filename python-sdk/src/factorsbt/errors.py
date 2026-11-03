"""Public exception types for SDK transport and contract failures."""


class FactorsBTError(Exception):
    """Base class for errors raised by the FactorsBT SDK."""


class TransportError(FactorsBTError):
    """The backend could not be reached or returned an unusable response."""


class RequestTimeout(TransportError):
    """The backend did not respond before the configured timeout."""


class HttpStatusError(FactorsBTError):
    """The backend returned an unsuccessful HTTP status."""

    def __init__(
        self,
        status: int,
        *,
        code: str | None = None,
        message: str | None = None,
        request_id: str | None = None,
        details: tuple[tuple[str, str], ...] = (),
    ):
        self.status = status
        self.code = code
        self.message = message
        self.request_id = request_id
        self.details = details
        description = f"backend returned HTTP {status}"
        if code:
            description += f" ({code})"
        super().__init__(description)


class ProtocolError(FactorsBTError):
    """The backend response did not match the health wire contract."""


class SubmissionAcknowledgementUnknown(TransportError):
    """A run POST ended without a trustworthy acknowledgment; look up its key."""

    def __init__(self, submission_id: str, reason: str):
        self.submission_id = submission_id
        self.reason = reason
        super().__init__(
            f"run submission acknowledgment is unknown; look up submission_id {submission_id}"
        )
