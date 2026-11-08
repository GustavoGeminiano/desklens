"""Bounded subprocess execution is essential when OCR engines hang."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

from run import run_native


class NativeProcessTests(unittest.TestCase):
    def test_native_thread_limit_is_explicit(self):
        run_native([sys.executable,'-c','import os; assert os.environ["OMP_THREAD_LIMIT"] == "1"'],timeout=5)

    def test_failure_is_not_a_successful_sample(self):
        with self.assertRaises(subprocess.CalledProcessError):
            run_native([sys.executable,'-c','raise SystemExit(7)'],timeout=5)

    def test_timeout_is_propagated(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            run_native([sys.executable,'-c','import time; time.sleep(60)'],timeout=.2)

    @unittest.skipUnless(sys.platform.startswith('linux'),'Linux process-state assertion')
    def test_timeout_stops_descendant_too(self):
        with tempfile.TemporaryDirectory() as directory:
            marker=Path(directory)/'child.pid'
            code='import pathlib,subprocess,sys,time; p=subprocess.Popen([sys.executable,"-c","import time; time.sleep(60)"]); pathlib.Path(sys.argv[1]).write_text(str(p.pid)); time.sleep(60)'
            with self.assertRaises(subprocess.TimeoutExpired):
                run_native([sys.executable,'-c',code,str(marker)],timeout=1)
            self.assertTrue(marker.exists(),'Descendant must have started before timeout')
            pid=int(marker.read_text())
            state=Path(f'/proc/{pid}/stat')
            # A terminated adopted child can briefly remain a zombie until init reaps it.
            deadline=time.monotonic()+2
            def terminated():
                try:
                    return state.read_text().split()[2] == 'Z'
                except FileNotFoundError:
                    # init may reap the child between an existence check and read.
                    return True
            while not terminated() and time.monotonic()<deadline:
                time.sleep(.02)
            self.assertTrue(terminated(),'OCR descendant survived process-group cleanup')


if __name__ == '__main__':
    unittest.main()
