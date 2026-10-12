"""Version checks shared by the runtime builder and archive packager."""

import re


def qt_version_at_least(version: str, minimum: str) -> bool:
    """Accept only numeric Qt major.minor.patch versions at or above minimum."""
    pattern = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+\Z")
    if not isinstance(version, str) or not isinstance(minimum, str):
        return False
    if not pattern.fullmatch(version) or not pattern.fullmatch(minimum):
        return False
    current_parts = tuple(map(int, version.split(".")))
    minimum_parts = tuple(map(int, minimum.split(".")))
    return current_parts[0] == minimum_parts[0] and current_parts >= minimum_parts
