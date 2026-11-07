import importlib.util
import struct
import json
import tempfile
from types import SimpleNamespace
from unittest.mock import patch
import unittest
from pathlib import Path

ROOT = Path(__file__).parent

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

native = load('native_guards', ROOT / 'm2-19-native.py')
recovery = load('recovery_guards', ROOT / 'm2-19-recovery.py')

class EditorGuards(unittest.TestCase):
    def identity(self, executable=b'/usr/bin/vim', argv=None):
        argv = argv or [b'/usr/bin/vim', b'-u', b'NONE', b'/proof/proof.txt']
        return {'executable_bytes': list(executable), 'kernel_executable_bytes': list(executable), 'argv_bytes': [list(a) for a in argv]}

    def test_exact_editor_and_operand(self):
        self.assertTrue(native.editor_identity_matches(self.identity(), {'executable':'/usr/bin/vim','operand':'/proof/proof.txt'}))

    def test_shell_mentions_do_not_authorize_editor_keystrokes(self):
        identity = self.identity(b'/bin/sh', [b'/bin/sh',b'-c',b'echo /usr/bin/vim /proof/proof.txt'])
        self.assertFalse(native.editor_identity_matches(identity, {'executable':'/usr/bin/vim','operand':'/proof/proof.txt'}))

    def test_operand_prefix_suffix_not_accepted(self):
        expected = {'executable':'/usr/bin/vim','operand':'/proof/proof.txt'}
        for operand in [b'/proof/proof.txt.old', b'/prefix/proof/proof.txt', b'--message=/proof/proof.txt']:
            self.assertFalse(native.editor_identity_matches(self.identity(argv=[b'/usr/bin/vim',operand]), expected))

    def test_missing_or_inconsistent_identity_rejected(self):
        expected = {'executable':'/usr/bin/vim','operand':'/proof/proof.txt'}
        self.assertFalse(native.editor_identity_matches(None, expected))
        self.assertFalse(native.editor_identity_matches({}, expected))
        identity = self.identity();identity['kernel_executable_bytes']=list(b'/bin/sh')
        self.assertFalse(native.editor_identity_matches(identity, expected))

    def test_nul_argv_spaces_and_bytes_round_trip_without_environment(self):
        argv = [b'/usr/bin/vim', b'/proof/file with spaces', b'raw-\xff']
        payload = struct.pack('=i', len(argv)) + b'/usr/bin/vim\0\0\0' + b'\0'.join(argv) + b'\0SECRET_ENV=not retained\0'
        executable, parsed = native.parse_darwin_procargs2(payload)
        self.assertEqual(executable, b'/usr/bin/vim');self.assertEqual(parsed, argv)

    def test_truncated_or_invalid_procargs_rejected(self):
        for payload in [b'', struct.pack('=i',0)+b'/bin/sh\0x\0', struct.pack('=i',2)+b'/bin/sh\0argv0\0']:
            with self.assertRaises(ValueError):native.parse_darwin_procargs2(payload)


class BoundaryGuards(unittest.TestCase):
    def test_clean_pinned_source_accepted(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            pin = Path(directory) / 'pin.json';pin.write_text(json.dumps({'source':'commit','binary_sha256':'hash','dirty':''}))
            args = SimpleNamespace(source=Path('/source'),binary=Path('/product'),pin=pin)
            with patch.object(native,'run',side_effect=lambda *args:'' if 'status' in args else 'commit'), patch.object(native,'sha',return_value='hash'):
                self.assertEqual(native.boundary(args)['dirty'],'')

    def test_stably_dirty_source_rejected_before_run(self):
        args = SimpleNamespace(source=Path('/source'),binary=Path('/product'),pin=None)
        with patch.object(native,'run',side_effect=lambda *args:' M src/file.rs' if 'status' in args else 'commit'), patch.object(native,'sha',return_value='hash'):
            with self.assertRaisesRegex(AssertionError,'source must be clean'):native.boundary(args)

    def test_dirty_pin_rejected_even_with_current_clean_source(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            pin = Path(directory) / 'pin.json';pin.write_text(json.dumps({'source':'commit','binary_sha256':'hash','dirty':'dirty'}))
            args = SimpleNamespace(source=Path('/source'),binary=Path('/product'),pin=pin)
            with patch.object(native,'run',side_effect=lambda *args:'' if 'status' in args else 'commit'), patch.object(native,'sha',return_value='hash'):
                with self.assertRaisesRegex(AssertionError,'pinned source must be clean'):native.boundary(args)


class RecoveryGuards(unittest.TestCase):
    workspace='11111111-1111-4111-8111-111111111111'
    request='22222222-2222-4222-8222-222222222222'

    def ref(self, role, suffix=''):
        return f'refs/rat/{role}/{self.workspace}/{self.request}{suffix}'

    def test_archive_index_tree_role_is_distinct(self):
        self.assertEqual(recovery.require_recovery_ref_type(self.ref('archive','-index'),'tree'),('archive-index','tree'))
        with self.assertRaises(ValueError):recovery.require_recovery_ref_type(self.ref('archive','-index'),'commit')
        with self.assertRaises(ValueError):recovery.require_recovery_ref_type(self.ref('archive'),'tree')

    def test_archive_prepared_tree_role_is_distinct(self):
        self.assertEqual(recovery.require_recovery_ref_type(self.ref('archive','-prepared'),'tree'),('archive-prepared','tree'))
        with self.assertRaises(ValueError):recovery.require_recovery_ref_type(self.ref('archive','-prepared'),'commit')
        with self.assertRaises(ValueError):recovery.require_recovery_ref_type(self.ref('archive','-prepared-index'),'tree')

    def test_creation_base_claim_requires_commit_and_exact_uuid_shape(self):
        name=f'refs/rat/creation/{self.request}'
        self.assertEqual(recovery.require_recovery_ref_type(name,'commit'),('creation-base-claim','commit'))
        with self.assertRaises(ValueError):recovery.require_recovery_ref_type(name,'tree')
        for invalid in [name+'-prepared',name+'/extra','refs/rat/creation/not-uuid']:
            with self.assertRaises(ValueError):recovery.recovery_ref_role(invalid)

    def test_primary_removal_and_slash_restore_refs_require_commits(self):
        for name in [self.ref('archive'),self.ref('removal'),self.ref('restore','/workspace/feature-index')]:
            self.assertEqual(recovery.require_recovery_ref_type(name,'commit')[1],'commit')
            with self.assertRaises(ValueError):recovery.require_recovery_ref_type(name,'tree')

    def test_unknown_ref_and_invalid_uuid_fail_closed(self):
        for name in [self.ref('unknown'),self.ref('restore'),self.ref('archive','-index/extra'),'refs/rat/archive/not-uuid/'+self.request]:
            with self.assertRaises(ValueError):recovery.recovery_ref_role(name)

    def observation(self, segment='segment-A', native='native-A', workspace='workspace-A'):
        return {'registry':{'selected':workspace},'session_identity':{'session':segment} if segment else None,'native_conversation':native}

    def test_exact_previous_selected_context_passes(self):
        old=self.observation();before={'segment-A':{'native_conversation':'native-A'}}
        self.assertIs(recovery.selected_context_preserved({'available':True,'observation':old},self.observation(),before),True)

    def test_different_old_selected_segment_or_workspace_fails(self):
        old=self.observation();before={'segment-A':{'native_conversation':'native-A'},'segment-B':{'native_conversation':'native-B'}}
        for new in [self.observation('segment-B','native-B'),self.observation(workspace='workspace-B'),self.observation(native='native-B')]:
            self.assertIs(recovery.selected_context_preserved({'available':True,'observation':old},new,before),False)

    def test_missing_previous_public_context_is_unverified(self):
        for old in [{},{'available':False},{'available':True},{'available':True,'observation':{'session_identity':{'session':'segment-A'}}}]:
            self.assertIsNone(recovery.selected_context_preserved(old,self.observation(),{}))

    def test_empty_selected_context_matches_only_same_workspace(self):
        old=self.observation(None,None)
        self.assertIs(recovery.selected_context_preserved({'available':True,'observation':old},self.observation(None,None),{}),True)
        self.assertIs(recovery.selected_context_preserved({'available':True,'observation':old},self.observation(None,None,'workspace-B'),{}),False)

if __name__ == '__main__':
    unittest.main()
