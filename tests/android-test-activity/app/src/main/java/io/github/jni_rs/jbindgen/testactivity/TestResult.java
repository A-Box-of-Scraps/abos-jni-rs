package io.github.jni_rs.jbindgen.testactivity;

final class TestResult {
    enum Status {
        PASSED,
        FAILED
    }

    final String name;
    final Status status;
    final String message;

    TestResult(String name, Status status, String message) {
        this.name = name;
        this.status = status;
        this.message = message;
    }
}
