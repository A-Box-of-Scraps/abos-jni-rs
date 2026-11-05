package io.github.jni_rs.jbindgen.testactivity;

import android.app.Activity;
import android.graphics.Color;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import java.util.UUID;

public class TestActivity extends Activity implements TestSession.NativeApi {
    private final Handler handler = new Handler(Looper.getMainLooper());
    private LinearLayout results;
    private TextView progress;
    private TestSession session;
    private TestReporter reporter;
    private String callback;

    public native void nativeOnCreate();

    public native String nativeGetMessage();

    public native int nativeProcessData(int value);

    public native int nativeGetTestCount();

    public native String nativeGetTestName(int index);

    public native String nativeRunTest(int index);

    // Deliberate Rust-to-Java callback test, separate from SDK test presentation.
    public void updateUi(String message) {
        callback = message;
    }

    @Override
    public String callbackMessage() {
        return callback;
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setupUi();
        String runId = getIntent().getStringExtra("abos.test.run-id");
        reporter = new TestReporter(runId == null ? UUID.randomUUID().toString() : runId);
        try {
            System.loadLibrary("testactivity");
            session = new TestSession(this);
            handler.post(this::runNext);
        } catch (RuntimeException | LinkageError | AssertionError error) {
            progress.setText("Test setup failed: " + error);
            reporter.setupFailure(error);
        }
    }

    @Override
    protected void onDestroy() {
        handler.removeCallbacksAndMessages(null);
        super.onDestroy();
    }

    private void runNext() {
        TestResult result = session.runNext();
        reporter.result(result);
        TextView line = new TextView(this);
        line.setText(result.status + " " + result.name + ": " + result.message);
        line.setTextColor(result.status == TestResult.Status.PASSED ? Color.rgb(0, 100, 0) : Color.RED);
        results.addView(line);
        progress.setText("Executed " + session.executed() + "/" + session.selected() + ", passed " + session.passed()
                + ", failed " + session.failed() + ", skipped 0");
        if (session.complete()) {
            reporter.summary(session);
        } else {
            handler.post(this::runNext);
        }
    }

    private void setupUi() {
        ScrollView scroll = new ScrollView(this);
        results = new LinearLayout(this);
        results.setOrientation(LinearLayout.VERTICAL);
        results.setBackgroundColor(Color.WHITE);
        int padding = (int) (16 * getResources().getDisplayMetrics().density);
        results.setPadding(padding, padding * 3, padding, padding * 2);
        progress = new TextView(this);
        progress.setText("Preparing JNI tests");
        progress.setTextColor(Color.BLACK);
        results.addView(progress);
        scroll.addView(results);
        setContentView(scroll);
    }
}
