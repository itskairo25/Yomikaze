import SwiftUI

struct CreditsView: View {
    struct CreditGroup: Identifiable {
        let role: String
        let icon: String
        let people: [String]

        var id: String { role }
    }

    static let groups: [CreditGroup] = [
        CreditGroup(role: "Lead Developer", icon: "hammer.fill", people: ["Kairo"]),
        CreditGroup(role: "Early Access Tester", icon: "testtube.2", people: ["illusive"]),
    ]

    var body: some View {
        ScrollView {
            VStack(spacing: 28) {
                VStack(spacing: 8) {
                    Text("読")
                        .font(.system(size: 64, weight: .bold))
                        .foregroundStyle(.tint)
                    Text("Yomikaze")
                        .font(.title2.weight(.bold))
                    Text(tr("Made with care by"))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .padding(.top, 12)

                ForEach(CreditsView.groups) { group in
                    VStack(alignment: .leading, spacing: 12) {
                        Label(tr(group.role), systemImage: group.icon)
                            .font(.footnote.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .textCase(.uppercase)

                        ForEach(group.people, id: \.self) { person in
                            HStack(spacing: 12) {
                                // the lead developer has a picture, everyone else gets their initial
                                if person == "Kairo" {
                                    Image("Kairo")
                                        .resizable()
                                        .scaledToFill()
                                        .frame(width: 40, height: 40)
                                        .clipShape(Circle())
                                } else {
                                    Text(String(person.prefix(1)).uppercased())
                                        .font(.headline)
                                        .foregroundStyle(.white)
                                        .frame(width: 40, height: 40)
                                        .background(.tint, in: Circle())
                                }
                                Text(person)
                                    .font(.title3.weight(.semibold))
                                Spacer(minLength: 0)
                            }
                        }
                    }
                    .padding(18)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
                }
            }
            .padding(.horizontal, 20)
            .padding(.bottom, 32)
        }
        .background {
            AmbientBackground()
        }
        .navigationTitle(tr("Credits"))
        .navigationBarTitleDisplayMode(.inline)
    }
}
