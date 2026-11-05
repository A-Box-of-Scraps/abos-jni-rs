package io.github.jni_rs.jbindgen.testactivity;

import android.util.Log;
import org.json.JSONException;
import org.json.JSONObject;

final class TestReporter {
    private final String runId;

    TestReporter(String runId) {
        this.runId = runId;
    }

    void result(TestResult result) {
        JSONObject event = event("result");
        put(event, "name", result.name);
        put(event, "status", result.status.name().toLowerCase(java.util.Locale.ROOT));
        // Bound individual log records so Android's log entry limit cannot truncate JSON.
        put(event, "message", result.message.substring(0, Math.min(200, result.message.length())));
        emit(event);
    }

    void summary(TestSession session) {
        summary(session.selected(), session.executed(), session.passed(), session.failed());
    }

    void setupFailure(Throwable error) {
        result(new TestResult("setup", TestResult.Status.FAILED, error.toString()));
        summary(1, 1, 0, 1);
    }

    private void summary(int selected, int executed, int passed, int failed) {
        JSONObject event = event("summary");
        put(event, "status", failed == 0 ? "passed" : "failed");
        put(event, "selected", selected);
        put(event, "executed", executed);
        put(event, "passed", passed);
        put(event, "failed", failed);
        put(event, "skipped", 0);
        emit(event);
    }

    private JSONObject event(String type) {
        JSONObject event = new JSONObject();
        put(event, "run_id", runId);
        put(event, "type", type);
        return event;
    }

    private static void put(JSONObject object, String name, Object value) {
        try {
            object.put(name, value);
        } catch (JSONException error) {
            throw new IllegalStateException("Cannot encode test report", error);
        }
    }

    private static void emit(JSONObject event) {
        Log.i("TestActivity", "ABOS_TEST_EVENT:" + event);
    }
}
