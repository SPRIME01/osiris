import pytest
from unittest import mock
from osiris.config import get_appdata_folder


def test_get_appdata_folder_windows(tmp_path):
    with mock.patch("sys.platform", "win32"):
        with mock.patch.dict("os.environ", {"APPDATA": str(tmp_path)}):
            expected_path = tmp_path / "osiris"
            assert get_appdata_folder() == str(expected_path)
            assert expected_path.exists()


def test_get_appdata_folder_windows_no_appdata():
    with mock.patch("sys.platform", "win32"):
        with mock.patch.dict("os.environ", {}, clear=True):
            with pytest.raises(
                EnvironmentError, match="APPDATA environment variable is not set."
            ):
                get_appdata_folder()


def test_get_appdata_folder_darwin(tmp_path):
    with mock.patch("sys.platform", "darwin"):
        with mock.patch("os.path.expanduser", return_value=str(tmp_path)):
            expected_path = tmp_path / "Library" / "Application Support" / "osiris"
            assert get_appdata_folder() == str(expected_path)
            assert expected_path.exists()


def test_get_appdata_folder_linux(tmp_path):
    with mock.patch("sys.platform", "linux"):
        with mock.patch("os.path.expanduser", return_value=str(tmp_path)):
            expected_path = tmp_path / ".local" / "share" / "osiris"
            assert get_appdata_folder() == str(expected_path)
            assert expected_path.exists()
