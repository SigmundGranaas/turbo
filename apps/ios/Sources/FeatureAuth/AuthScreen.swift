import SwiftUI
import CoreAuth
import CoreDesignSystem

/// Sign in — hero map, wordmark, and the Apple / Email options. Mirrors
/// `LoginScreen` (design) / `feature.auth.AuthScreen` (Android).
public struct AuthScreen: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    @State private var viewModel: AuthViewModel
    private let onSignedIn: () -> Void

    public init(viewModel: AuthViewModel, onSignedIn: @escaping () -> Void = {}) {
        _viewModel = State(initialValue: viewModel)
        self.onSignedIn = onSignedIn
    }

    public var body: some View {
        VStack(spacing: 0) {
            hero
            VStack(spacing: 0) {
                Text("Turbo")
                    .font(.system(.largeTitle, design: .default, weight: .heavy))
                    .foregroundStyle(t.label)
                Text("The hiking map made for Norwegian mountains.")
                    .font(.turboBody)
                    .foregroundStyle(t.label2)
                    .multilineTextAlignment(.center)
                    .padding(.top, 6)

                VStack(spacing: 12) {
                    Button(action: viewModel.signIn) {
                        HStack(spacing: 10) {
                            Text("G")
                                .font(.system(size: 18, weight: .bold))
                                .foregroundStyle(t.blue)
                                .frame(width: 24, height: 24)
                                .background(.white, in: Circle())
                            Text("Continue with Google").font(.turboHeadline)
                        }
                        .frame(maxWidth: .infinity, minHeight: 52)
                        .background(t.label, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                        .foregroundStyle(t.background)
                    }
                    .accessibilityIdentifier("auth.google")
                    Button(action: viewModel.signIn) {
                        Label("Continue with Email", systemImage: "envelope.fill")
                            .font(.turboHeadline)
                            .frame(maxWidth: .infinity, minHeight: 52)
                            .background(t.fill3, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                            .foregroundStyle(t.label)
                    }
                }
                .padding(.top, 28)
                .disabled(viewModel.isWorking)

                Text("By continuing you agree to the Terms and Privacy Policy.")
                    .font(.turboFootnote)
                    .foregroundStyle(t.label2)
                    .multilineTextAlignment(.center)
                    .padding(.top, 18)
                Spacer()
            }
            .padding(.horizontal, 28)
            .padding(.top, 8)
        }
        .background(t.background)
        .task { viewModel.start() }
        .onChange(of: viewModel.state) { _, state in
            if state.account != nil { onSignedIn(); dismiss() }
        }
    }

    private var hero: some View {
        ZStack(alignment: .bottom) {
            LinearGradient(
                colors: [Color(hex: 0xAAD3DF), t.background],
                startPoint: .top, endPoint: .bottom
            )
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(LinearGradient(colors: [Color(hex: 0x0A84FF), Color(hex: 0x0A4FA8)],
                                     startPoint: .topLeading, endPoint: .bottomTrailing))
                .frame(width: 72, height: 72)
                .overlay(Image(systemName: "mountain.2.fill").font(.system(size: 34, weight: .semibold)).foregroundStyle(.white))
                .shadow(color: Color(hex: 0x0A84FF).opacity(0.4), radius: 12, y: 8)
                .padding(.bottom, 18)
        }
        .frame(height: 320)
        .clipped()
    }
}
