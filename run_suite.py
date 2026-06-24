"""
    run_suite.py
    By: DrkWithT
"""

import os
import subprocess

FERROJS_TEST_SUITE_DIR = os.path.relpath('./suite')
FERROJS_TEST_SUITE_GROUPS = ['basics'] # TODO add 'objects' and 'builtins'
FERROJS_TEST_PROCESS_COUNT = 4;

def get_test_names(test_suite_path: str = FERROJS_TEST_SUITE_DIR, folders: list[str] = FERROJS_TEST_SUITE_GROUPS) -> list[str]:
    all_test_names = []

    for folder_name in folders:
        current_folder_prefix = f'{test_suite_path}/{folder_name}'
        current_folder_entries = os.listdir(current_folder_prefix)
        all_test_names += [
            f'{current_folder_prefix}/{test_case_filename}'
            for test_case_filename
            in current_folder_entries
        ]
    
    return all_test_names

def run_tests_by_n(executable_path: str, test_file_paths: list[str], worker_count: int = FERROJS_TEST_PROCESS_COUNT):
    total_tests = len(test_file_paths)
    total_passed = 0

    while test_file_paths:
        batch = [(
            test_path,
            f'{executable_path} -r {test_path}'
        ) for test_path in test_file_paths[:worker_count]]
        test_file_paths = test_file_paths[worker_count:]

        batched_procs = [subprocess.Popen(
            named_test[1],
            shell=True,
            stdout=None,
        ) for named_test in batch]

        for batch_test_id, test_cmd in enumerate(batched_procs):
            if test_cmd.wait() == 0:
                print(f'Test \x1b[1;33m{batch[batch_test_id][0]}\x1b[0m:  \x1b[1;32mPASS\x1b[0m')
                total_passed += 1
            else:
                print(f'Test \x1b[1;33m{batch[batch_test_id][0]}\x1b[0m:  \x1b[1;31mFAIL\x1b[0m')

    return (total_passed, total_tests - total_passed, total_tests)   

if __name__ == '__main__':
    executable_path = None;

    if os.path.exists("./target/debug/ferrojs"):
        executable_path = "./target/debug/ferrojs"
    elif os.path.exists("./target/release/ferrojs"):
        executable_path = "./target/release/ferrojs"
    else:
        print("Missing \x1b[1;31mFerroJS\x1b[0m executable. Try building with \x1b[1;33mcargo b\x1b[0m")
        exit(1)

    pass_count, fail_count, test_count = run_tests_by_n(
        executable_path,
        get_test_names()
    )

    print(f'\nTEST REPORT:\n\x1b[1;34mPASSED:\x1b[0m {pass_count}/{test_count}\n\x1b[1;34mFAILED:\x1b[0m {fail_count}/{test_count}')

    exit(0 if fail_count == 0 else 1)
