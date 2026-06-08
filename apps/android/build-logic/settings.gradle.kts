// Composite build hosting the Turbo convention plugins. Included by the root
// settings via `pluginManagement { includeBuild("build-logic") }` so the
// `turbo.android.*` plugin ids resolve in every module.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
    versionCatalogs {
        create("libs") {
            from(files("../gradle/libs.versions.toml"))
        }
    }
}

rootProject.name = "build-logic"
include(":convention")
