package com.jbindgen;

import com.sun.source.tree.*;
import com.sun.source.util.*;
import java.io.IOException;
import java.util.*;
import javax.tools.*;

/** Prepares Android SDK documentation stubs for compiler type analysis. */
final class AndroidStubs {
    private AndroidStubs() {}

    static List<JavaFileObject> prepare(
            JavaCompiler compiler,
            StandardJavaFileManager fileManager,
            List<JavaFileObject> units,
            List<String> options)
            throws IOException {
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        JavacTask task = (JavacTask) compiler.getTask(null, fileManager, diagnostics, options, null, units);
        SourcePositions positions = Trees.instance(task).getSourcePositions();
        List<JavaFileObject> result = new ArrayList<>();
        for (CompilationUnitTree tree : task.parse()) {
            StringBuilder content = new StringBuilder(tree.getSourceFile().getCharContent(true));
            Map<String, String> importedNames = new HashMap<>();
            for (ImportTree node : tree.getImports()) {
                String name = node.getQualifiedIdentifier().toString();
                String simpleName = name.substring(name.lastIndexOf('.') + 1);
                if (!simpleName.equals("*")) {
                    importedNames.put(simpleName, name);
                }
            }
            Set<String> identifiers = new HashSet<>();
            new TreeScanner<Void, Void>() {
                @Override
                public Void visitAnnotation(AnnotationTree node, Void p) {
                    String name = node.getAnnotationType().toString();
                    // Resolve explicit imports before filtering, including imported
                    // enclosing types used in qualified nested annotation names.
                    int separator = name.indexOf('.');
                    String firstName = separator < 0 ? name : name.substring(0, separator);
                    String importedName = importedNames.get(firstName);
                    if (importedName != null) {
                        name = importedName + (separator < 0 ? "" : name.substring(separator));
                    }
                    // SDK documentation retains these annotations even when android.jar
                    // omits them or makes them package-private. They do not affect JNI
                    // signatures. Keep Java annotations (especially Deprecated) and
                    // jbindgen annotations, which do carry binding metadata.
                    if (name.startsWith("android.annotation.") || name.startsWith("androidx.annotation.")) {
                        erase(content, tree, node, positions);
                        return null;
                    }
                    return super.visitAnnotation(node, p);
                }

                @Override
                public Void visitImport(ImportTree node, Void p) {
                    return null;
                }

                @Override
                public Void visitIdentifier(IdentifierTree node, Void p) {
                    identifiers.add(node.getName().toString());
                    return super.visitIdentifier(node, p);
                }
            }.scan(tree, null);
            // Documentation stubs can retain unused imports of hidden implementation
            // types. Only remove imports with no remaining source references; used
            // imports and unresolved API types must still produce compiler errors.
            for (ImportTree node : tree.getImports()) {
                String name = node.getQualifiedIdentifier().toString();
                String simpleName = name.substring(name.lastIndexOf('.') + 1);
                if (!simpleName.equals("*") && !identifiers.contains(simpleName)) {
                    erase(content, tree, node, positions);
                }
            }
            JavaFileObject source = tree.getSourceFile();
            result.add(new SimpleJavaFileObject(source.toUri(), JavaFileObject.Kind.SOURCE) {
                @Override
                public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                    return content;
                }

                @Override
                public String getName() {
                    return source.getName();
                }
            });
        }
        for (Diagnostic<? extends JavaFileObject> diagnostic : diagnostics.getDiagnostics()) {
            if (diagnostic.getKind() == Diagnostic.Kind.ERROR) {
                throw new IOException(diagnostic.toString());
            }
        }
        return result;
    }

    private static void erase(StringBuilder content, CompilationUnitTree unit, Tree node, SourcePositions positions) {
        int start = (int) positions.getStartPosition(unit, node);
        int end = (int) positions.getEndPosition(unit, node);
        // Preserve source positions, line numbers, and Javadoc in diagnostics/output.
        for (int i = start; i < end; i++) {
            if (content.charAt(i) != '\n' && content.charAt(i) != '\r') {
                content.setCharAt(i, ' ');
            }
        }
    }
}
