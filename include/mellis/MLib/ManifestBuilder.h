#ifndef MELLIS_MLIB_MANIFESTBUILDER_H
#define MELLIS_MLIB_MANIFESTBUILDER_H

#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/StringTableBuilder.h"
#include <string>
#include <vector>

namespace fl {
namespace mlib {

class ManifestBuilder {
public:
    explicit ManifestBuilder(StringTableBuilder& stringTable);

    void setPackageName(const std::string& name);
    void setAuthor(const std::string& author);
    void setVersion(const std::string& version);
    void setLicense(const std::string& license);
    void addFeature(const std::string& feature);

    // Dependency management
    void addDependency(const uint8_t uuid[16], const std::string& version, uint64_t hash, ImportMode mode, const std::vector<std::string>& features);

    // Write to binary
    void serialize(BinaryWriter& writer) const;

private:
    StringTableBuilder& stringTable;
    
    uint32_t nameStringID = 0;
    uint32_t authorStringID = 0;
    uint32_t versionStringID = 0;
    uint32_t licenseStringID = 0;
    
    std::vector<uint32_t> features; // feature string IDs

    struct DepInfo {
        DependencyEntry entry;
        std::vector<uint32_t> featureIDs;
    };
    std::vector<DepInfo> dependencies;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_MANIFESTBUILDER_H
