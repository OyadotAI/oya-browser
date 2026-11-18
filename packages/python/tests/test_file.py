"""file(): a task value carrying a file's bytes, its name and its MIME type."""

import base64

import pytest

from oya_browser import MAX_FILE_BYTES, file


def test_a_path_becomes_its_name_its_type_from_the_extension_and_its_bytes(tmp_path):
    path = tmp_path / "cv.PDF"
    path.write_bytes(b"%PDF")
    assert file(path) == {"file": "cv.PDF", "type": "application/pdf", "b64": base64.b64encode(b"%PDF").decode()}


def test_raw_bytes_are_named_file_and_typed_as_octet_stream():
    assert file(b"x") == {"file": "file", "type": "application/octet-stream", "b64": "eA=="}


def test_name_and_type_override_what_the_source_suggests():
    value = file(b"x", name="photo.png", type="image/x-custom")
    assert (value["file"], value["type"]) == ("photo.png", "image/x-custom")


def test_a_name_with_a_known_extension_sets_the_type():
    assert file(b"x", name="sheet.xlsx")["type"].endswith("spreadsheetml.sheet")


def test_a_file_over_the_limit_is_refused_saying_how_big_it_is():
    with pytest.raises(ValueError, match=r"big\.bin is 11MB; the limit for a task file is 10MB"):
        file(b"0" * (MAX_FILE_BYTES + 1024 * 1024), name="big.bin")


def test_a_file_exactly_at_the_limit_is_accepted():
    assert file(b"0" * MAX_FILE_BYTES)["file"] == "file"
